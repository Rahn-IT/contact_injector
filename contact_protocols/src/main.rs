use std::env::var;

use contact_protocols::{
    ContactDestination,
    caldav_birthdays::{CaldavAccessData, CaldavBirthdayDestination},
    carddav::CarddavError,
};
use hyper_rustls::HttpsConnectorBuilder;

#[tokio::main]
async fn main() {
    let caldav_url = var("CALDAV_URL").unwrap();
    let caldav_username = var("CALDAV_USERNAME").unwrap();
    let caldav_password = var("CALDAV_PASSWORD").unwrap();

    let access_data = CaldavAccessData {
        url: caldav_url,
        username: caldav_username,
        password: caldav_password,
    };

    let destination = CaldavBirthdayDestination::new(access_data).await.unwrap();
    destination.export_contacts([].iter()).await.unwrap();
}
