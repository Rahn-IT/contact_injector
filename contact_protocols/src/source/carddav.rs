//! Adapter for CardDAV address books.

use dav_client::carddav_client::CardDavClient;
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::contact::Contact;

use super::ContactSource;

/// Raw connection settings serialized at the application boundary.
#[derive(Serialize, Deserialize)]
pub struct AccessData {
    pub url: String,
    pub username: String,
    pub password: String,
}

/// A connected address book ready to fetch contacts.
pub struct Source {
    client: CardDavClient,
}

/// A failed adapter operation with its underlying cause.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("inner error: {0}")]
    Inner(#[from] dav_client::carddav_client::CardDavError),
    #[error("invalid addressbook URI: {0:?}")]
    InvalidAddressbookUri(#[from] url::ParseError),
}

impl Source {
    /// Connects using the supplied settings and checks remote access.
    pub async fn new(access_data: AccessData) -> Result<Self, Error> {
        let addressbook_uri: Url = access_data.url.parse()?;
        let username = access_data.username;
        let password = access_data.password;

        let client = CardDavClient::new(addressbook_uri, &username, &password)?;
        client.list_contacts().await?;

        Ok(Self { client })
    }
}

impl ContactSource for Source {
    type Error = Error;

    async fn fetch_contacts(&self) -> Result<Vec<crate::contact::Contact>, Self::Error> {
        let resource_list = self.client.list_contacts().await?;

        let contacts = self
            .client
            .fetch_contacts(&resource_list)
            .await?
            .iter()
            .map(Contact::from_vcard)
            .collect();

        Ok(contacts)
    }
}
