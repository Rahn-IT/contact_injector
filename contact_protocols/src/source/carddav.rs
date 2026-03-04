use dav_client::carddav_client::CardDavClient;
use http::StatusCode;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::contact::Contact;

use super::ContactSource;

#[derive(Serialize, Deserialize)]
pub struct CarddavAccessData {
    pub url: String,
    pub username: String,
    pub password: String,
}

pub struct CarddavSource {
    client: CardDavClient,
    addressbook_url: Url,
}

#[derive(Debug, Error)]
pub enum CarddavError {
    #[error("inner error: {0}")]
    Inner(#[from] dav_client::carddav_client::CardDavError),
    #[error("fetch error: {0}")]
    FetchError(StatusCode),
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("invalid addressbook URI: {0:?}")]
    InvalidAddressbookUri(#[from] url::ParseError),
}

impl CarddavSource {
    pub async fn new(access_data: CarddavAccessData) -> Result<Self, CarddavError> {
        let addressbook_uri: Url = access_data.url.parse()?;
        let username = access_data.username;
        let password = access_data.password;

        let client = CardDavClient::new(addressbook_uri.clone(), &username, &password)?;
        client.list_contacts().await?;

        Ok(Self {
            client,
            addressbook_url: addressbook_uri,
        })
    }
}

impl ContactSource for CarddavSource {
    type Error = CarddavError;

    async fn fetch_contacts(&self) -> Result<Vec<crate::contact::Contact>, Self::Error> {
        let resource_list = self.client.list_contacts().await?;

        let contacts = self
            .client
            .fetch_contacts(&resource_list)
            .await?
            .iter()
            .map(Contact::from_vcard)
            .collect();

        // let contacts = resource_list
        //     .resources
        //     .iter()
        //     .map(|card| {
        //         // Todo: detect unfinished parsing and return error
        //         Ok(parse_vcard(
        //             &card
        //                 .content
        //                 .as_ref()
        //                 .map_err(|err| CarddavError::FetchError(err.clone()))?
        //                 .data,
        //         )
        //         .map_err(|e| CarddavError::ParseError(e.to_string()))?
        //         .1)
        //     })
        //     .collect::<Result<Vec<_>, CarddavError>>()?;

        Ok(contacts)
    }
}
