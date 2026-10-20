//! HTTP entry point and application initialization.

use std::sync::Arc;

use axum::{
    Router,
    extract::State,
    response::Html,
    routing::{get, post},
};
use http::{HeaderValue, header};

use sqlx::{Sqlite, SqlitePool, migrate::MigrateDatabase};

use crate::error::Error;

mod destinations;
mod error;
mod job;
mod sources;
mod state;
mod sync_jobs;

const DB_PATH: &str = "./db/db.sqlite";

fn router() -> Router<state::State> {
    Router::new()
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
            get(destinations::starface::new_get),
        )
        .route(
            "/destinations/new/starface",
            post(destinations::starface::new_post),
        )
        .route(
            "/destinations/edit/starface/{id}",
            get(destinations::starface::edit_get),
        )
        .route(
            "/destinations/edit/starface/{id}",
            post(destinations::starface::edit_post),
        )
        .route(
            "/destinations/new/caldav",
            get(destinations::caldav::new_get),
        )
        .route(
            "/destinations/new/caldav",
            post(destinations::caldav::new_post),
        )
        .route(
            "/destinations/edit/caldav/{id}",
            get(destinations::caldav::edit_get),
        )
        .route(
            "/destinations/edit/caldav/{id}",
            post(destinations::caldav::edit_post),
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // initialize tracing
    tracing_subscriber::fmt::init();

    if !tokio::fs::try_exists(DB_PATH).await? {
        tokio::fs::create_dir_all(
            std::path::Path::new(DB_PATH)
                .parent()
                .expect("database path has a parent"),
        )
        .await?;
        Sqlite::create_database(DB_PATH).await?;
    }

    let db = SqlitePool::connect(DB_PATH).await?;
    sqlx::migrate!("./migrations").run(&db).await?;

    let mut jinja = minijinja::Environment::new();
    minijinja_embed::load_templates!(&mut jinja);

    let jobs = sqlx::query_scalar!("SELECT id FROM jobs")
        .fetch_all(&db)
        .await?;

    let state = state::State {
        db,
        jobs: job::Manager::default(),
        jinja: Arc::new(jinja),
    };

    for id in jobs {
        state
            .jobs
            .start(&state, id)
            .await
            .map_err(Error::into_report)?;
    }

    let app = router().with_state(state);

    let addr = "0.0.0.0:4040";
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "starting contact injector");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn root(State(state): State<state::State>) -> Result<Html<String>, Error> {
    let template = state.jinja.get_template("home.html")?;
    let rendered = template.render(())?;
    Ok(Html(rendered))
}
