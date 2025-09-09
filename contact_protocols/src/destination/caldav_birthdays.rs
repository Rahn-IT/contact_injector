use http::{Uri, uri::InvalidUri};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use libdav::{CalDavClient, dav::WebDavClient};
use serde::{Deserialize, Serialize};
use tower_http::auth::AddAuthorization;

use crate::{ContactDestination, carddav::CarddavError, contact::Contact};

#[derive(Serialize, Deserialize)]
pub struct CaldavAccessData {
    pub url: String,
    pub username: String,
    pub password: String,
}

pub struct CaldavBirthdayDestination {
    client: CalDavClient<AddAuthorization<Client<HttpsConnector<HttpConnector>, String>>>,
    calendar_uri: Uri,
}

#[derive(Debug, thiserror::Error)]
pub enum CaldavError {
    #[error("error while loading root certs: {0}")]
    LoadRootError(std::io::Error),
    #[error("inner error: {0}")]
    Inner(#[from] libdav::dav::WebDavError),
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("invalid addressbook URI: {0:?}")]
    InvalidAddressbookUri(#[from] InvalidUri),
}

impl CaldavBirthdayDestination {
    pub async fn new(access_data: CaldavAccessData) -> Result<Self, CaldavError> {
        let calendar_uri: Uri = access_data.url.parse()?;
        let username = access_data.username;
        let password = access_data.password;

        let https_connector = HttpsConnectorBuilder::new()
            .with_native_roots()
            .map_err(CaldavError::LoadRootError)?
            .https_or_http()
            .enable_http1()
            .build();
        let http_client = Client::builder(TokioExecutor::new()).build(https_connector);
        let auth_client = AddAuthorization::basic(http_client, &username, &password);
        let webdav = WebDavClient::new(calendar_uri.clone(), auth_client);

        let client = libdav::CalDavClient::new(webdav);
        client.list_resources(calendar_uri.path()).await?;

        Ok(Self {
            client,
            calendar_uri,
        })
    }
}

impl ContactDestination for CaldavBirthdayDestination {
    type Error = CaldavError;

    async fn export_contacts(
        &self,
        contacts: impl Iterator<Item = &Contact>,
    ) -> Result<(), Self::Error> {
        todo!()
    }
}
