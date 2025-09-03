use anyhow::anyhow;
use axum::{
    Form,
    extract::{Path, State},
    response::{Html, Redirect},
};
use contact_protocols::{
    ContactSource,
    carddav::{CarddavAccessData, CarddavSource},
    contact::Contact,
};
use serde::{Deserialize, Serialize};

use crate::{AppError, AppState};

#[derive(Serialize, Debug)]
pub struct Source {
    id: i64,
    name: String,
    source_type: String,
    access_data: String,
}

#[derive(Serialize)]
pub struct SourceList {
    sources: Vec<Source>,
}

pub async fn list(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let sources = sqlx::query_as!(Source, "SELECT * FROM sources")
        .fetch_all(&state.db)
        .await?;

    let template = state
        .jinja
        .get_template("sources.html")
        .expect("template is loaded");
    let rendered = template.render(SourceList { sources }).unwrap();
    Ok(Html(rendered))
}

#[derive(Serialize, Deserialize)]
pub struct CarddavSourceForm {
    name: String,
    url: String,
    username: String,
    password: String,
}

pub async fn new_carddav_get(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    carddav_edit(state, None)
}

pub async fn new_carddav_post(
    State(state): State<AppState>,
    Form(form): Form<CarddavSourceForm>,
) -> Result<Redirect, AppError> {
    let access_data = CarddavAccessData {
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
    .await
    .unwrap();

    Ok(Redirect::to("/sources"))
}

pub async fn edit_carddav_get(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let source = sqlx::query_as!(Source, "SELECT * FROM sources WHERE id = ?", id)
        .fetch_one(&state.db)
        .await?;

    let access_data: CarddavAccessData = serde_json::from_str(&source.access_data)?;

    let form_data = CarddavSourceForm {
        name: source.name,
        url: access_data.url,
        username: access_data.username,
        password: access_data.password,
    };

    carddav_edit(state, Some(form_data))
}

pub async fn edit_carddav_post(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<CarddavSourceForm>,
) -> Result<Redirect, AppError> {
    let access_data = CarddavAccessData {
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
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError> {
    sqlx::query!("DELETE FROM sources WHERE id = ?", id)
        .execute(&state.db)
        .await?;

    Ok(Redirect::to("/sources"))
}

fn carddav_edit(
    state: AppState,
    data: Option<CarddavSourceForm>,
) -> Result<Html<String>, AppError> {
    let template = state
        .jinja
        .get_template("carddav_source.html")
        .expect("template is loaded");
    let rendered = template.render(data).unwrap();
    Ok(Html(rendered))
}

pub async fn poll_source(state: &AppState, id: i64) -> Result<Vec<Contact>, AppError> {
    let source_data = sqlx::query!(
        "SELECT source_type, access_data FROM sources WHERE id = ?",
        id
    )
    .fetch_one(&state.db)
    .await?;

    match source_data.source_type.as_str() {
        "carddav" => {
            let access_data: CarddavAccessData = serde_json::from_str(&source_data.access_data)?;
            let source = CarddavSource::new(access_data).await?;

            source
                .fetch_contacts()
                .await
                .map_err(|err| anyhow!(err).into())
        }
        source_type => Err(anyhow!("Source type not found: {}", source_type).into()),
    }
}
