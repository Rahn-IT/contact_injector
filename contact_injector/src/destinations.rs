use anyhow::anyhow;
use axum::{
    extract::{Path, State},
    response::{Html, Redirect},
};
use contact_protocols::{
    ContactDestination,
    caldav_birthdays::{CaldavAccessData, CaldavBirthdayDestination},
    contact::Contact,
    starface::{StarfaceAccessData, StarfaceDestination},
};
use serde::Serialize;

use crate::{AppError, AppState};

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
pub struct DestinationList {
    destinations: Vec<Destination>,
}

pub async fn list(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let destinations = sqlx::query_as!(Destination, "SELECT * FROM destinations")
        .fetch_all(&state.db)
        .await?;

    let template = state
        .jinja
        .get_template("destinations.html")
        .expect("template is loaded");
    let rendered = template.render(DestinationList { destinations }).unwrap();
    Ok(Html(rendered))
}

pub async fn delete_destination(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError> {
    sqlx::query!("DELETE FROM destinations WHERE id = ?", id)
        .execute(&state.db)
        .await?;

    Ok(Redirect::to("/destinations"))
}

pub async fn export_to_destination(
    state: &AppState,
    id: i64,
    contacts: Vec<Contact>,
) -> Result<(), AppError> {
    let destination = sqlx::query_as!(Destination, "SELECT * FROM destinations WHERE id = ?", id)
        .fetch_one(&state.db)
        .await?;

    match destination.destination_type.as_str() {
        "starface" => {
            let access_data: StarfaceAccessData = serde_json::from_str(&destination.access_data)?;
            let destination = StarfaceDestination::new(access_data).await?;

            destination
                .export_contacts(contacts.iter())
                .await
                .map_err(|err| anyhow!(err).into())
        }
        "caldav" => {
            let access_data: CaldavAccessData = serde_json::from_str(&destination.access_data)?;
            let destination = CaldavBirthdayDestination::new(access_data).await?;

            destination
                .export_contacts(contacts.iter())
                .await
                .map_err(|err| anyhow!(err).into())
        }
        destination_type => Err(anyhow!("Destination type unknown: {}", destination_type).into()),
    }
}
