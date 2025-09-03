use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    Form,
    extract::{Path, State},
    response::{Html, Redirect},
};
use serde::{Deserialize, Serialize};
use tokio::{select, sync::mpsc};
use tokio_util::sync::CancellationToken;

use crate::{
    AppError, AppState, JobHandle, JobInfo, destinations::export_to_destination,
    sources::poll_source,
};

#[derive(Serialize)]
pub struct JobItem {
    id: i64,
    name: String,
    source: String,
    destination: String,
    delay: i64,
    info: JobInfo,
}

#[derive(Serialize)]
pub struct JobList {
    jobs: Vec<JobItem>,
}

#[derive(Serialize)]
struct JobTemplateData {
    form: Option<JobForm>,
    sources: Vec<(i64, String)>,
    destinations: Vec<(i64, String)>,
}

#[derive(Serialize, Deserialize)]
pub struct JobForm {
    name: String,
    source: i64,
    destination: i64,
    delay: i64,
}

pub async fn list(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let jobs = sqlx::query!("SELECT jobs.id, jobs.name, jobs.delay, sources.name as source, destinations.name as destination FROM jobs JOIN sources ON jobs.source = sources.id JOIN destinations ON jobs.destination = destinations.id")
        .fetch_all(&state.db)
        .await?;

    let guard = state.job_map.lock().unwrap();

    let jobs = jobs
        .into_iter()
        .map(|job| JobItem {
            id: job.id,
            name: job.name,
            delay: job.delay,
            source: job.source,
            destination: job.destination,
            info: guard.get(&job.id).unwrap().info.lock().unwrap().clone(),
        })
        .collect();

    let template = state
        .jinja
        .get_template("jobs.html")
        .expect("template is loaded");
    let rendered = template.render(JobList { jobs }).unwrap();
    Ok(Html(rendered))
}

pub async fn new_job_get(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    job_edit(state, None).await
}

pub async fn new_job_post(
    State(state): State<AppState>,
    Form(data): Form<JobForm>,
) -> Result<Redirect, AppError> {
    let id = sqlx::query_scalar!(
        "INSERT INTO jobs (name, delay, source, destination) VALUES (?, ?, ?, ?) RETURNING id",
        data.name,
        data.delay,
        data.source,
        data.destination
    )
    .fetch_one(&state.db)
    .await?
    .unwrap();

    start_job(&state, id).await?;

    Ok(Redirect::to("/jobs"))
}

pub async fn edit_job_get(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let job = sqlx::query!(
        "SELECT name, delay, source, destination FROM jobs WHERE id = ?",
        id
    )
    .fetch_one(&state.db)
    .await?;

    job_edit(
        state,
        Some(JobForm {
            name: job.name,
            delay: job.delay,
            source: job.source,
            destination: job.destination,
        }),
    )
    .await
}

pub async fn edit_job_post(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(data): Form<JobForm>,
) -> Result<Redirect, AppError> {
    sqlx::query!(
        "UPDATE jobs SET name = ?, delay = ?, source = ?, destination = ? WHERE id = ?",
        data.name,
        data.delay,
        data.source,
        data.destination,
        id
    )
    .execute(&state.db)
    .await?;

    Ok(Redirect::to("/jobs"))
}

pub async fn delete_job(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError> {
    stop_job(&state, id).await;

    sqlx::query!("DELETE FROM jobs WHERE id = ?", id)
        .execute(&state.db)
        .await?;

    Ok(Redirect::to("/jobs"))
}

pub async fn run_now(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError> {
    {
        let guard = state.job_map.lock().unwrap();
        if let Some(job) = guard.get(&id) {
            let _ = job.run_now.try_send(());
        }
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    Ok(Redirect::to("/jobs"))
}

// pub async fn run_now(
//     State(state): State<AppState>,
//     Path(id): Path<i64>,
// ) -> Result<Redirect, AppError> {
//     let guard = state.job_map.lock().unwrap();
//     if let Some(job) = guard.get(&id) {
//         let _ = job.run_now.try_send(());
//     }
//     drop(guard);
//     tokio::time::sleep(Duration::from_millis(100)).await;
//     Ok(Redirect::to("/jobs"))
// }

async fn job_edit(state: AppState, data: Option<JobForm>) -> Result<Html<String>, AppError> {
    let destinations = sqlx::query!("SELECT id, name from destinations")
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(|record| (record.id, record.name))
        .collect();

    let sources = sqlx::query!("SELECT id, name from sources")
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(|record| (record.id, record.name))
        .collect();

    let template = state
        .jinja
        .get_template("job.html")
        .expect("template is loaded");
    let rendered = template
        .render(JobTemplateData {
            form: data,
            sources,
            destinations,
        })
        .unwrap();
    Ok(Html(rendered))
}

pub async fn start_job(state: &AppState, id: i64) -> Result<(), AppError> {
    let cancel_token = CancellationToken::new();
    let (send, recv) = mpsc::channel(1);
    let info = Arc::new(Mutex::new(JobInfo {
        currently_running: false,
        last_log: "-".into(),
        last_run: "-".into(),
    }));

    let handle = JobHandle {
        cancel_token: cancel_token.clone(),
        info: info.clone(),
        run_now: send,
    };

    {
        let mut guard = state.job_map.lock().unwrap();
        guard.insert(id, handle);
    }

    let state_clone = state.clone();
    tokio::spawn(async move {
        let mut recv = recv;
        loop {
            {
                let mut guard = info.lock().unwrap();
                guard.currently_running = true;
            }
            println!("Running job {}", id);
            let run_result = run_sync(&state_clone, id).await;
            {
                let mut guard = info.lock().unwrap();
                guard.currently_running = false;
                guard.last_log = match &run_result {
                    Ok(_) => "Success".to_string(),
                    Err(e) => format!("Error: {}", e),
                };
                guard.last_run = format!("{} UTC", chrono::Utc::now().format("%d/%m/%Y %H:%M:%S"))
            }
            let delay = match sqlx::query!("SELECT delay FROM jobs WHERE id = ?", id)
                .fetch_optional(&state_clone.db)
                .await
            {
                Ok(Some(job)) => job.delay as u64,
                Ok(None) => return,
                Err(e) => {
                    eprintln!("Failed to fetch job {}: {}", id, e);
                    return;
                }
            };
            let exit = select! {
                // normal delay
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(delay * 60)) => {false}
                // run now
                _ = recv.recv() => {false}
                // stop sync job
                _ = cancel_token.cancelled() => {true}
            };

            if exit {
                return;
            }
        }
    });

    Ok(())
}

async fn stop_job(state: &AppState, id: i64) {
    let guard = state.job_map.lock().unwrap();
    if let Some(handle) = guard.get(&id) {
        handle.cancel_token.cancel();
    };
}

#[must_use]
async fn run_sync(state: &AppState, id: i64) -> Result<(), AppError> {
    let job = sqlx::query!("SELECT source, destination FROM jobs WHERE id = ?", id)
        .fetch_one(&state.db)
        .await?;

    let contacts = poll_source(&state, job.source).await?;
    export_to_destination(&state, job.destination, contacts).await?;

    Ok(())
}
