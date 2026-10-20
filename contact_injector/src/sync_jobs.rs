use axum::{
    Form,
    extract::{Path, State},
    response::{Html, Redirect},
};
use serde::{Deserialize, Serialize};

use crate::{error, job, state};

#[derive(Serialize)]
pub struct Item {
    id: i64,
    name: String,
    source: String,
    destination: String,
    delay: i64,
    info: job::Info,
}

#[derive(Serialize)]
pub struct List {
    jobs: Vec<Item>,
}

#[derive(Serialize)]
struct TemplateData {
    form: Option<FormData>,
    sources: Vec<(i64, String)>,
    destinations: Vec<(i64, String)>,
}

#[derive(Serialize, Deserialize)]
pub struct FormData {
    name: String,
    source: i64,
    destination: i64,
    delay: job::delay::Delay,
}

pub async fn list(State(state): State<state::State>) -> Result<Html<String>, error::Error> {
    let jobs = sqlx::query!("SELECT jobs.id, jobs.name, jobs.delay, sources.name as source, destinations.name as destination FROM jobs JOIN sources ON jobs.source = sources.id JOIN destinations ON jobs.destination = destinations.id")
        .fetch_all(&state.db)
        .await?;

    let mut items = Vec::with_capacity(jobs.len());
    for job in jobs {
        items.push(Item {
            id: job.id,
            name: job.name,
            delay: job.delay,
            source: job.source,
            destination: job.destination,
            info: state.jobs.info(job.id).await,
        });
    }
    let jobs = items;

    let template = state.jinja.get_template("jobs.html")?;
    let rendered = template.render(List { jobs })?;
    Ok(Html(rendered))
}

pub async fn new_job_get(State(state): State<state::State>) -> Result<Html<String>, error::Error> {
    job_edit(state, None).await
}

pub async fn new_job_post(
    State(state): State<state::State>,
    Form(data): Form<FormData>,
) -> Result<Redirect, error::Error> {
    let id = sqlx::query_scalar!(
        "INSERT INTO jobs (name, delay, source, destination) VALUES (?, ?, ?, ?) RETURNING id",
        data.name,
        i64::from(data.delay),
        data.source,
        data.destination
    )
    .fetch_one(&state.db)
    .await?
    .ok_or_else(|| anyhow::anyhow!("inserted job did not return an ID"))?;

    state.jobs.start(&state, id).await?;

    Ok(Redirect::to("/jobs"))
}

pub async fn edit_job_get(
    State(state): State<state::State>,
    Path(id): Path<i64>,
) -> Result<Html<String>, error::Error> {
    let job = sqlx::query!(
        "SELECT name, delay, source, destination FROM jobs WHERE id = ?",
        id
    )
    .fetch_one(&state.db)
    .await?;

    job_edit(
        state,
        Some(FormData {
            name: job.name,
            delay: job::delay::Delay::try_from(job.delay)?,
            source: job.source,
            destination: job.destination,
        }),
    )
    .await
}

pub async fn edit_job_post(
    State(state): State<state::State>,
    Path(id): Path<i64>,
    Form(data): Form<FormData>,
) -> Result<Redirect, error::Error> {
    sqlx::query!(
        "UPDATE jobs SET name = ?, delay = ?, source = ?, destination = ? WHERE id = ?",
        data.name,
        i64::from(data.delay),
        data.source,
        data.destination,
        id
    )
    .execute(&state.db)
    .await?;

    Ok(Redirect::to("/jobs"))
}

pub async fn delete_job(
    State(state): State<state::State>,
    Path(id): Path<i64>,
) -> Result<Redirect, error::Error> {
    state.jobs.stop(id).await?;

    sqlx::query!("DELETE FROM jobs WHERE id = ?", id)
        .execute(&state.db)
        .await?;

    Ok(Redirect::to("/jobs"))
}

pub async fn run_now(
    State(state): State<state::State>,
    Path(id): Path<i64>,
) -> Result<Redirect, error::Error> {
    state.jobs.run_now(id).await;
    Ok(Redirect::to("/jobs"))
}

async fn job_edit(
    state: state::State,
    data: Option<FormData>,
) -> Result<Html<String>, error::Error> {
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

    let template = state.jinja.get_template("job.html")?;
    let rendered = template.render(TemplateData {
        form: data,
        sources,
        destinations,
    })?;
    Ok(Html(rendered))
}

#[cfg(test)]
mod tests {
    use axum::{Form, body::Body, extract::FromRequest, http::Request};

    use super::FormData;

    #[tokio::test]
    async fn form_extraction_accepts_valid_minutes_and_rejects_invalid_intervals() {
        for (minutes, valid) in [("5", true), ("0", false), ("-1", false), ("525601", false)] {
            let request = Request::builder()
                .method("POST")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!(
                    "name=test&source=1&destination=1&delay={minutes}"
                )))
                .unwrap();
            let result = Form::<FormData>::from_request(request, &()).await;
            assert_eq!(result.is_ok(), valid, "interval {minutes}");
        }
    }
}
