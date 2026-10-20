use anyhow::anyhow;
use axum::{
    extract::{Path, State},
    response::{Html, Redirect},
};
use contact_protocols::{ContactDestination, caldav_birthdays, contact::Contact};
use serde::Serialize;

use crate::{error, state};

pub mod caldav;
pub mod starface;

#[derive(Serialize, Debug)]
pub struct Destination {
    id: i64,
    name: String,
    destination_type: String,
    access_data: String,
}

#[derive(Serialize)]
pub struct List {
    destinations: Vec<Destination>,
}

pub async fn list(State(state): State<state::State>) -> Result<Html<String>, error::Error> {
    let destinations = sqlx::query_as!(Destination, "SELECT * FROM destinations")
        .fetch_all(&state.db)
        .await?;

    let template = state.jinja.get_template("destinations.html")?;
    let rendered = template.render(List { destinations })?;
    Ok(Html(rendered))
}

pub async fn delete_destination(
    State(state): State<state::State>,
    Path(id): Path<i64>,
) -> Result<Redirect, error::Error> {
    sqlx::query!("DELETE FROM destinations WHERE id = ?", id)
        .execute(&state.db)
        .await?;

    Ok(Redirect::to("/destinations"))
}

pub async fn export_to_destination(
    state: &state::State,
    id: i64,
    contacts: Vec<Contact>,
) -> Result<(), error::Error> {
    let destination = sqlx::query_as!(Destination, "SELECT * FROM destinations WHERE id = ?", id)
        .fetch_one(&state.db)
        .await?;

    match destination.destination_type.as_str() {
        "starface" => {
            let access_data: contact_protocols::starface::AccessData =
                serde_json::from_str(&destination.access_data)?;
            let destination = contact_protocols::starface::Destination::new(access_data).await?;

            destination
                .export_contacts(contacts.iter())
                .await
                .map_err(|err| anyhow!(err).into())
        }
        "caldav" => {
            let access_data: caldav_birthdays::AccessData =
                serde_json::from_str(&destination.access_data)?;
            let destination = caldav_birthdays::Destination::new(access_data).await?;

            destination
                .export_contacts(contacts.iter())
                .await
                .map_err(|err| anyhow!(err).into())
        }
        destination_type => Err(anyhow!("Destination type unknown: {}", destination_type).into()),
    }
}
