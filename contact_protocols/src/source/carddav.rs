use http::{StatusCode, Uri, uri::InvalidUri};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use libdav::{
    CardDavClient,
    carddav::{GetAddressBookResources, get_addressbook_resources},
    dav::{WebDavClient, WebDavError},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tower_http::auth::AddAuthorization;

use super::ContactSource;
use crate::vcard::parse_vcard;

#[derive(Serialize, Deserialize)]
pub struct CarddavAccessData {
    pub url: String,
    pub username: String,
    pub password: String,
}

pub struct CarddavSource {
    client: CardDavClient<AddAuthorization<Client<HttpsConnector<HttpConnector>, String>>>,
    addressbook_uri: Uri,
}

#[derive(Debug, Error)]
pub enum CarddavError<C> {
    #[error("error while loading root certs: {0}")]
    LoadRootError(std::io::Error),
    #[error("inner error: {0}")]
    Inner(#[from] WebDavError<C>),
    #[error("fetch error: {0}")]
    FetchError(StatusCode),
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("invalid addressbook URI: {0:?}")]
    InvalidAddressbookUri(#[from] InvalidUri),
}

impl CarddavSource {
    pub async fn new(access_data: CarddavAccessData) -> Result<Self, CarddavError<_>> {
        let addressbook_uri: Uri = access_data.url.parse()?;
        let username = access_data.username;
        let password = access_data.password;

        let https_connector = HttpsConnectorBuilder::new()
            .with_native_roots()
            .map_err(CarddavError::LoadRootError)?
            .https_or_http()
            .enable_http1()
            .build();
        let http_client = Client::builder(TokioExecutor::new()).build(https_connector);
        let auth_client = AddAuthorization::basic(http_client, &username, &password);
        let webdav = WebDavClient::new(addressbook_uri.clone(), auth_client);

        let client = libdav::CardDavClient::new(webdav);
        match client
            .request(GetAddressBookResources::new(addressbook_uri.path()))
            .await {
                Ok(items) => (),
                Err(err)
            }

        Ok(Self {
            client,
            addressbook_uri,
        })
    }
}

impl ContactSource for CarddavSource {
    type Error = CarddavError;

    async fn fetch_contacts(&self) -> Result<Vec<crate::contact::Contact>, Self::Error> {
        let resource_list = self
            .client
            .request(GetAddressBookResources::new(self.addressbook_uri.path()))
            .await?;

        let contacts = resource_list
            .resources
            .iter()
            .map(|card| {
                // Todo: detect unfinished parsing and return error
                Ok(parse_vcard(
                    &card
                        .content
                        .as_ref()
                        .map_err(|err| CarddavError::FetchError(err.clone()))?
                        .data,
                )
                .map_err(|e| CarddavError::ParseError(e.to_string()))?
                .1)
            })
            .collect::<Result<Vec<_>, CarddavError>>()?;

        Ok(contacts)
    }
}
