//! Ownership and communication for scheduled sync workers.

use std::{collections::HashMap, sync::Arc};

use serde::Serialize;
use tokio::{
    select,
    sync::{Mutex, mpsc, watch},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::{destinations, error, sources, state};

pub mod delay;

/// Owns live workers; callers communicate through channels.
#[derive(Debug, Clone, Default)]
pub struct Manager(Arc<Mutex<HashMap<i64, Handle>>>);

#[derive(Debug)]
struct Handle {
    cancel: CancellationToken,
    run_now: mpsc::Sender<()>,
    info: watch::Receiver<Info>,
    task: JoinHandle<()>,
}

/// The latest published status of a sync worker.
#[derive(Debug, Clone, Serialize)]
pub struct Info {
    last_run: String,
    last_log: String,
    currently_running: bool,
}

impl Default for Info {
    fn default() -> Self {
        Self {
            last_run: "-".into(),
            last_log: "-".into(),
            currently_running: false,
        }
    }
}

impl Manager {
    /// Starts a worker only if its stored interval is valid and it is not live.
    pub async fn start(&self, state: &state::State, id: i64) -> Result<(), error::Error> {
        let minutes = sqlx::query_scalar!("SELECT delay FROM jobs WHERE id = ?", id)
            .fetch_one(&state.db)
            .await?;
        let _delay = delay::Delay::try_from(minutes)?;
        let mut workers = self.0.lock().await;
        if workers.contains_key(&id) {
            return Ok(());
        }
        let cancel = CancellationToken::new();
        let (send, recv) = mpsc::channel(1);
        let (info, snapshot) = watch::channel(Info::default());
        let task = tokio::spawn(run(state.clone(), id, cancel.clone(), recv, info));
        workers.insert(
            id,
            Handle {
                cancel,
                run_now: send,
                info: snapshot,
                task,
            },
        );
        Ok(())
    }

    /// Publishes the latest status without exposing a worker's mutable state.
    pub async fn info(&self, id: i64) -> Info {
        self.0
            .lock()
            .await
            .get(&id)
            .map(|handle| handle.info.borrow().clone())
            .unwrap_or_default()
    }

    /// Queues at most one additional run.
    pub async fn run_now(&self, id: i64) {
        if let Some(handle) = self.0.lock().await.get(&id) {
            let _ = handle.run_now.try_send(());
        }
    }

    /// Stops scheduling and waits for an active export to finish before removal.
    pub async fn stop(&self, id: i64) -> Result<(), error::Error> {
        let handle = self.0.lock().await.remove(&id);
        if let Some(handle) = handle {
            handle.cancel.cancel();
            handle.task.await?;
        }
        Ok(())
    }
}

async fn run(
    state: state::State,
    id: i64,
    cancel: CancellationToken,
    mut requests: mpsc::Receiver<()>,
    info: watch::Sender<Info>,
) {
    loop {
        if cancel.is_cancelled() {
            return;
        }
        info.send_modify(|status| status.currently_running = true);
        tracing::info!(id, "running sync job");
        let result = sync(&state, id).await;
        info.send_modify(|status| {
            status.currently_running = false;
            status.last_log = match result {
                Ok(()) => "Success".into(),
                Err(error) => format!("Error: {error}"),
            };
            status.last_run = format!("{} UTC", chrono::Utc::now().format("%d/%m/%Y %H:%M:%S"));
        });
        let delay = async {
            let minutes = sqlx::query_scalar!("SELECT delay FROM jobs WHERE id = ?", id)
                .fetch_one(&state.db)
                .await?;
            Ok::<_, error::Error>(delay::Delay::try_from(minutes)?)
        }
        .await;
        let delay = match delay {
            Ok(delay) => delay,
            Err(error) => {
                tracing::error!(id, %error, "sync worker stopped");
                info.send_modify(|status| status.last_log = format!("Stopped: {error}"));
                return;
            }
        };
        select! {
            biased;
            _ = cancel.cancelled() => return,
            request = requests.recv() => if request.is_none() { return; },
            _ = tokio::time::sleep(delay.duration()) => {},
        }
    }
}

async fn sync(state: &state::State, id: i64) -> Result<(), error::Error> {
    let job = sqlx::query!("SELECT source, destination FROM jobs WHERE id = ?", id)
        .fetch_one(&state.db)
        .await?;
    let contacts = sources::poll_source(state, job.source).await?;
    destinations::export_to_destination(state, job.destination, contacts).await
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use super::Manager;
    use crate::state;

    async fn fixture(minutes: i64) -> state::State {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&db).await.unwrap();
        // An unsupported source exercises failure reporting without external I/O.
        sqlx::query("INSERT INTO sources VALUES (1, 'test', 'unsupported', '{}')")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO destinations VALUES (1, 'test', 'unsupported', '{}')")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO jobs VALUES (1, 'test', 1, 1, ?)")
            .bind(minutes)
            .execute(&db)
            .await
            .unwrap();
        state::State {
            db,
            jinja: Arc::new(minijinja::Environment::new()),
            jobs: Manager::default(),
        }
    }

    #[tokio::test]
    async fn invalid_stored_delay_never_starts_a_worker() {
        let state = fixture(0).await;
        assert!(state.jobs.start(&state, 1).await.is_err());
        assert!(state.jobs.0.lock().await.is_empty());
    }

    #[tokio::test]
    async fn workers_publish_failures_and_stop_without_waiting_for_the_interval() {
        let state = fixture(525_600).await;
        state.jobs.start(&state, 1).await.unwrap();
        state.jobs.start(&state, 1).await.unwrap();
        assert_eq!(state.jobs.0.lock().await.len(), 1);
        let mut info = state.jobs.0.lock().await.get(&1).unwrap().info.clone();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if info.borrow_and_update().last_log.starts_with("Error:") {
                    break;
                }
                info.changed().await.unwrap();
            }
        })
        .await
        .unwrap();
        assert!(!info.borrow().currently_running);
        tokio::time::timeout(Duration::from_secs(5), state.jobs.stop(1))
            .await
            .unwrap()
            .unwrap();
        assert!(state.jobs.0.lock().await.is_empty());
        assert!(info.changed().await.is_err());
    }
}
