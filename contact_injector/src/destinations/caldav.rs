use axum::{
    Form,
    extract::{Path, State},
    response::{Html, Redirect},
};
use contact_protocols::caldav_birthdays::CaldavAccessData;
use serde::{Deserialize, Serialize};

use crate::{AppError, AppState};

#[derive(Serialize, Deserialize)]
pub struct CaldavDestinationForm {
    name: String,
    url: String,
    username: String,
    password: String,
}

pub async fn new_get(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    caldav_edit(state, None)
}

pub async fn new_post(
    State(state): State<AppState>,
    Form(form): Form<CaldavDestinationForm>,
) -> Result<Redirect, AppError> {
    let access_data = CaldavAccessData {
        url: form.url,
        username: form.username,
        password: form.password,
    };

    let access_data_json = serde_json::to_string(&access_data)?;

    sqlx::query!(
        "INSERT INTO destinations (name, destination_type, access_data) VALUES (?, ?, ?)",
        form.name,
        "caldav",
        access_data_json
    )
    .execute(&state.db)
    .await
    .unwrap();

    Ok(Redirect::to("/destinations"))
}

pub async fn edit_get(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError> {
    let destination = sqlx::query!(
        "SELECT name, access_data FROM destinations WHERE id = ?",
        id
    )
    .fetch_one(&state.db)
    .await?;

    let access_data: CaldavAccessData = serde_json::from_str(&destination.access_data)?;

    let form_data = CaldavDestinationForm {
        name: destination.name,
        url: access_data.url,
        username: access_data.username,
        password: access_data.password,
    };

    caldav_edit(state, Some(form_data))
}

pub async fn edit_post(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<CaldavDestinationForm>,
) -> Result<Redirect, AppError> {
    let access_data = CaldavAccessData {
        url: form.url,
        username: form.username,
        password: form.password,
    };

    let access_data_json = serde_json::to_string(&access_data)?;

    sqlx::query!(
        "UPDATE destinations SET name = ?, access_data = ? WHERE id = ?",
        form.name,
        access_data_json,
        id
    )
    .execute(&state.db)
    .await?;

    Ok(Redirect::to("/destinations"))
}

fn caldav_edit(
    state: AppState,
    data: Option<CaldavDestinationForm>,
) -> Result<Html<String>, AppError> {
    let template = state
        .jinja
        .get_template("caldav_destination.html")
        .expect("template is loaded");
    let rendered = template.render(data).unwrap();
    Ok(Html(rendered))
}
