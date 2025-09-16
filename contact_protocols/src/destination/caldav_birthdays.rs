use std::{collections::HashMap, fmt::Display, hash::Hash};

use chrono::{Days, Utc};
use http::{Uri, uri::InvalidUri};
use hyper_rustls::{HttpsConnector, HttpsConnectorBuilder};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use itertools::Itertools;
use libdav::{CalDavClient, dav::WebDavClient};
use serde::{Deserialize, Serialize};
use tower_http::auth::AddAuthorization;
use uuid::Uuid;

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
    #[error("todo")]
    Todo,
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
        println!("Exporting birthdays to CalDAV");
        let resources = self.client.list_resources(self.calendar_uri.path()).await?;
        let entries = self
            .client
            .get_calendar_resources(
                self.calendar_uri.path(),
                resources.iter().map(|r| r.href.as_str()),
            )
            .await?;

        // println!("Resources: {:#?}", resources);

        let entries = entries
            .iter()
            .map(|entry| entry.content.as_ref().unwrap().data.as_str())
            .map(ICS::from_data)
            .filter(ICS::is_mine);

        let mut found = HashMap::new();

        for entry in entries {
            found.insert(entry, false);
        }

        println!("Found {} existing entries", found.len());

        for birthday in contacts
            .filter_map(|contact| ICS::birthday(contact))
            .take(1)
        {
            // if found.contains_key(&birthday) {
            if found.keys().next() == Some(&birthday) {
                println!("=====Found birthday:=====\n{}", birthday);
                found.insert(birthday, true);
            } else {
                println!("=====Adding birthday:=====\n{}", birthday);
                self.add_ics(birthday).await?;
            }
        }

        Err(CaldavError::Todo)
    }
}

impl CaldavBirthdayDestination {
    async fn add_ics(&self, ics: ICS) -> Result<(), CaldavError> {
        let id = ics.id().unwrap();

        let url = format!("{}{}", self.calendar_uri.path(), id);
        println!("Adding ICS to URL: {}", url);

        self.client
            .create_resource(&url, ics.0.into_bytes(), b"text/calendar")
            .await?;

        Ok(())
    }
}

struct ICS(String);

impl Display for ICS {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ICS {
    fn from_data(data: &str) -> Self {
        #[allow(unstable_name_collisions)]
        let data: String = data
            .trim()
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.len() > 0 {
                    Some(trimmed)
                } else {
                    None
                }
            })
            .intersperse("\n")
            .collect();

        Self(data)
    }

    fn birthday(contact: &Contact) -> Option<Self> {
        let birthday = contact.birthday.as_ref()?;
        let start = birthday.format("%Y%m%d").to_string();
        let end = birthday
            .checked_add_days(Days::new(1))
            .unwrap()
            .format("%Y%m%d")
            .to_string();

        let uid = Uuid::new_v4().to_string();
        let now = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();

        let name = &contact.display_name;

        let ics = format!(
            "\
            BEGIN:VCALENDAR
            PRODID:-//Rahn-IT/ContactInjector//EN
            VERSION:2.0
            BEGIN:VEVENT
            UID:{uid}
            SUMMARY:{name}
            TRANSP:OPAQUE
            DTSTART;VALUE=DATE:{start}
            DTEND;VALUE=DATE:{end}
            CREATED:{now}
            DTSTAMP:{now}
            LAST-MODIFIED:{now}
            BEGIN:VALARM
            TRIGGER;RELATED=START;VALUE=DURATION:-P0D
            ACTION:DISPLAY
            SUMMARY:{name}
            DESCRIPTION:{name}
            END:VALARM
            RRULE:FREQ=YEARLY
            SEQUENCE:1
            CLASS:CONFIDENTIAL
            END:VEVENT
            END:VCALENDAR
            "
        );

        Some(Self::from_data(&ics))
    }

    fn is_mine(&self) -> bool {
        self.0.contains("PRODID:-//Rahn-IT/ContactInjector//EN")
    }

    fn filtered_lines(&self) -> impl Iterator<Item = &str> {
        self.0.lines().filter(|line| {
            !line.starts_with("CREATED:")
                && !line.starts_with("DTSTAMP:")
                && !line.starts_with("LAST-MODIFIED:")
                && !line.starts_with("UID:")
                && !line.starts_with("SEQUENCE:")
        })
    }

    fn id(&self) -> Option<String> {
        let id_line = self.0.lines().find(|line| line.starts_with("UID:"))?;
        let id = id_line[4..].to_string();
        Some(id)
    }
}

impl Hash for ICS {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for line in self.filtered_lines() {
            line.hash(state);
        }
    }
}

impl PartialEq for ICS {
    fn eq(&self, other: &Self) -> bool {
        for line in self.filtered_lines().zip(other.filtered_lines()) {
            if line.0 != line.1 {
                println!("Lines differ: {} != {}", line.0, line.1);
                return false;
            }
        }
        true
    }
}

impl Eq for ICS {}
