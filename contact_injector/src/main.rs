use std::{
    collections::HashMap,
    fmt::Display,
    path::Path,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use http::{HeaderValue, header};

use serde::Serialize;
use sqlx::{Sqlite, SqlitePool, migrate::MigrateDatabase};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::sync_jobs::start_job;

mod destinations;
mod sources;
mod sync_jobs;

const DB_PATH: &str = "./db/db.sqlite";

fn router() -> Router<AppState> {
    Router::new()
        // `GET /` goes to `root`
        .route("/", get(root))
        .route("/sources", get(sources::list))
        .route("/sources/new/carddav", get(sources::new_carddav_get))
        .route("/sources/new/carddav", post(sources::new_carddav_post))
        .route("/sources/edit/carddav/{id}", get(sources::edit_carddav_get))
        .route(
            "/sources/edit/carddav/{id}",
            post(sources::edit_carddav_post),
        )
        .route("/sources/delete/{id}", post(sources::delete_source))
        .route("/destinations", get(destinations::list))
        .route(
            "/destinations/new/starface",
            get(destinations::new_starface_get),
        )
        .route(
            "/destinations/new/starface",
            post(destinations::new_starface_post),
        )
        .route(
            "/destinations/edit/starface/{id}",
            get(destinations::edit_starface_get),
        )
        .route(
            "/destinations/edit/starface/{id}",
            post(destinations::edit_starface_post),
        )
        .route(
            "/destinations/delete/{id}",
            post(destinations::delete_destination),
        )
        .route("/jobs", get(sync_jobs::list))
        .route("/jobs/new", get(sync_jobs::new_job_get))
        .route("/jobs/new", post(sync_jobs::new_job_post))
        .route("/jobs/run_now/{id}", get(sync_jobs::run_now))
        .route("/jobs/edit/{id}", get(sync_jobs::edit_job_get))
        .route("/jobs/edit/{id}", post(sync_jobs::edit_job_post))
        .route("/jobs/delete/{id}", post(sync_jobs::delete_job))
        .route(
            "/static/style.css",
            get((
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static(mime::TEXT_CSS_UTF_8.as_ref()),
                )],
                include_bytes!("../assets/static/style.css"),
            )),
        )
}

#[derive(Debug, Clone)]
struct AppState {
    db: SqlitePool,
    jinja: Arc<minijinja::Environment<'static>>,
    job_map: Arc<Mutex<HashMap<i64, JobHandle>>>,
}

#[derive(Debug)]
pub struct JobHandle {
    cancel_token: CancellationToken,
    run_now: mpsc::Sender<()>,
    info: Arc<Mutex<JobInfo>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobInfo {
    last_run: String,
    last_log: String,
    currently_running: bool,
}

#[derive(Debug)]
struct AppError(anyhow::Error);
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();

    if !tokio::fs::try_exists(DB_PATH).await.unwrap() {
        tokio::fs::create_dir_all(Path::new(DB_PATH).parent().unwrap())
            .await
            .unwrap();
        Sqlite::create_database(DB_PATH).await.unwrap();
    }

    let db = SqlitePool::connect(DB_PATH).await.unwrap();
    sqlx::migrate!("./migrations").run(&db).await.unwrap();

    let mut jinja = minijinja::Environment::new();
    minijinja_embed::load_templates!(&mut jinja);

    let jobs = sqlx::query_scalar!("SELECT id FROM jobs")
        .fetch_all(&db)
        .await
        .unwrap();

    let state = AppState {
        db,
        job_map: Arc::new(Mutex::new(HashMap::new())),
        jinja: Arc::new(jinja),
    };

    for id in jobs {
        start_job(&state, id).await.unwrap();
    }

    // build our application with a route
    let app = router().with_state(state);

    // run our app with hyper, listening globally on port 3000
    let addr = "0.0.0.0:4040";
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("Starting contact injector on: {}", addr);
    axum::serve(listener, app).await.unwrap();
}

async fn root(State(state): State<AppState>) -> Html<String> {
    let template = state
        .jinja
        .get_template("home.html")
        .expect("template is loaded");
    let rendered = template.render(()).unwrap();
    Html(rendered)
}
