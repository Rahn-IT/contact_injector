use axum::{
    Form,
    extract::{Path, State},
    response::{Html, Redirect},
};
use contact_protocols::caldav_birthdays;
use serde::{Deserialize, Serialize};

use crate::{error, state};

#[derive(Serialize, Deserialize)]
pub struct FormData {
    name: String,
    url: String,
    username: String,
    password: String,
}

pub async fn new_get(State(state): State<state::State>) -> Result<Html<String>, error::Error> {
    caldav_edit(state, None)
}

pub async fn new_post(
    State(state): State<state::State>,
    Form(form): Form<FormData>,
) -> Result<Redirect, error::Error> {
    let access_data = caldav_birthdays::AccessData {
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
    .await?;

    Ok(Redirect::to("/destinations"))
}

pub async fn edit_get(
    State(state): State<state::State>,
    Path(id): Path<i64>,
) -> Result<Html<String>, error::Error> {
    let destination = sqlx::query!(
        "SELECT name, access_data FROM destinations WHERE id = ?",
        id
    )
    .fetch_one(&state.db)
    .await?;

    let access_data: caldav_birthdays::AccessData = serde_json::from_str(&destination.access_data)?;

    let form_data = FormData {
        name: destination.name,
        url: access_data.url,
        username: access_data.username,
        password: access_data.password,
    };

    caldav_edit(state, Some(form_data))
}

pub async fn edit_post(
    State(state): State<state::State>,
    Path(id): Path<i64>,
    Form(form): Form<FormData>,
) -> Result<Redirect, error::Error> {
    let access_data = caldav_birthdays::AccessData {
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

fn caldav_edit(state: state::State, data: Option<FormData>) -> Result<Html<String>, error::Error> {
    let template = state.jinja.get_template("caldav_destination.html")?;
    let rendered = template.render(data)?;
    Ok(Html(rendered))
}
