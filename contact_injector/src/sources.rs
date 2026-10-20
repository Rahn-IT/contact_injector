use anyhow::anyhow;
use axum::{
    Form,
    extract::{Path, State},
    response::{Html, Redirect},
};
use contact_protocols::{ContactSource, carddav, contact::Contact};
use serde::{Deserialize, Serialize};

use crate::{error, state};

#[derive(Serialize, Debug)]
pub struct Source {
    id: i64,
    name: String,
    source_type: String,
    access_data: String,
}

#[derive(Serialize)]
pub struct List {
    sources: Vec<Source>,
}

pub async fn list(State(state): State<state::State>) -> Result<Html<String>, error::Error> {
    let sources = sqlx::query_as!(Source, "SELECT * FROM sources")
        .fetch_all(&state.db)
        .await?;

    let template = state.jinja.get_template("sources.html")?;
    let rendered = template.render(List { sources })?;
    Ok(Html(rendered))
}

#[derive(Serialize, Deserialize)]
pub struct FormData {
    name: String,
    url: String,
    username: String,
    password: String,
}

pub async fn new_carddav_get(
    State(state): State<state::State>,
) -> Result<Html<String>, error::Error> {
    carddav_edit(state, None)
}

pub async fn new_carddav_post(
    State(state): State<state::State>,
    Form(form): Form<FormData>,
) -> Result<Redirect, error::Error> {
    let access_data = carddav::AccessData {
        url: form.url,
        username: form.username,
        password: form.password,
    };

    let access_data_json = serde_json::to_string(&access_data)?;

    sqlx::query!(
        "INSERT INTO sources (name, source_type, access_data) VALUES (?, ?, ?)",
        form.name,
        "carddav",
        access_data_json
    )
    .execute(&state.db)
    .await?;

    Ok(Redirect::to("/sources"))
}

pub async fn edit_carddav_get(
    State(state): State<state::State>,
    Path(id): Path<i64>,
) -> Result<Html<String>, error::Error> {
    let source = sqlx::query_as!(Source, "SELECT * FROM sources WHERE id = ?", id)
        .fetch_one(&state.db)
        .await?;

    let access_data: carddav::AccessData = serde_json::from_str(&source.access_data)?;

    let form_data = FormData {
        name: source.name,
        url: access_data.url,
        username: access_data.username,
        password: access_data.password,
    };

    carddav_edit(state, Some(form_data))
}

pub async fn edit_carddav_post(
    State(state): State<state::State>,
    Path(id): Path<i64>,
    Form(form): Form<FormData>,
) -> Result<Redirect, error::Error> {
    let access_data = carddav::AccessData {
        url: form.url,
        username: form.username,
        password: form.password,
    };

    let access_data_json = serde_json::to_string(&access_data)?;

    sqlx::query!(
        "UPDATE sources SET name = ?, access_data = ? WHERE id = ?",
        form.name,
        access_data_json,
        id
    )
    .execute(&state.db)
    .await?;

    Ok(Redirect::to("/sources"))
}

pub async fn delete_source(
    State(state): State<state::State>,
    Path(id): Path<i64>,
) -> Result<Redirect, error::Error> {
    sqlx::query!("DELETE FROM sources WHERE id = ?", id)
        .execute(&state.db)
        .await?;

    Ok(Redirect::to("/sources"))
}

fn carddav_edit(state: state::State, data: Option<FormData>) -> Result<Html<String>, error::Error> {
    let template = state.jinja.get_template("carddav_source.html")?;
    let rendered = template.render(data)?;
    Ok(Html(rendered))
}

pub async fn poll_source(state: &state::State, id: i64) -> Result<Vec<Contact>, error::Error> {
    let source_data = sqlx::query!(
        "SELECT source_type, access_data FROM sources WHERE id = ?",
        id
    )
    .fetch_one(&state.db)
    .await?;

    match source_data.source_type.as_str() {
        "carddav" => {
            let access_data: carddav::AccessData = serde_json::from_str(&source_data.access_data)?;
            let source = carddav::Source::new(access_data).await?;

            source
                .fetch_contacts()
                .await
                .map_err(|err| anyhow!(err).into())
        }
        source_type => Err(anyhow!("Source type not found: {}", source_type).into()),
    }
}
