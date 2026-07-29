use crate::contact::Contact;
use dav_client::caldav_client::CalDavClient;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::ContactDestination;

#[derive(Serialize, Deserialize)]
pub struct CaldavAccessData {
    pub url: String,
    pub username: String,
    pub password: String,
}

pub struct CaldavBirthdayDestination {
    client: CalDavClient,
    calendar_uri: Url,
}

#[derive(Debug, thiserror::Error)]
pub enum CaldavError {
    #[error("error while loading root certs: {0}")]
    LoadRootError(std::io::Error),
    #[error("inner error: {0}")]
    Inner(#[from] dav_client::caldav_client::CalDavError),
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("invalid addressbook URI: {0:?}")]
    InvalidAddressbookUri(#[from] url::ParseError),
    #[error("todo")]
    Todo,
}

impl CaldavBirthdayDestination {
    pub async fn new(access_data: CaldavAccessData) -> Result<Self, CaldavError> {
        let calendar_url: Url = access_data.url.parse()?;
        let username = access_data.username;
        let password = access_data.password;

        let client = CalDavClient::new(calendar_url.clone(), &username, &password)?;

        client.list_calendar_entries().await?;

        Ok(Self {
            client,
            calendar_uri: calendar_url,
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
        let refs = self.client.list_calendar_entries().await?;
        let entries = self.client.fetch_calendar_entries(&refs).await?;

        println!("Found {} existing entries", entries.len());

        // Resource creation and birthday reconciliation are not implemented yet.
        // Consume the iterator so callers can still supply any iterator type
        // without leaving misleading partial synchronization behavior in place.
        let _birthday_count = contacts.filter(|contact| contact.birthday.is_some()).count();

        Err(CaldavError::Todo)
    }
}

/*
TODO: Restore this prototype once `ICalendar` can be serialized and
`CalDavClient::create_resource` has been implemented.

The prototype previously used these additional imports:

    use std::collections::HashMap;
    use chrono::{Days, Utc};
    use dav_client::vobject::icalendar::ICalendar;
    use uuid::Uuid;

Its intended reconciliation loop was:

    let mut found = HashMap::new();

    for entry in entries {
        found.insert(entry, false);
    }

    for birthday in contacts
        .filter_map(|contact| ICS::birthday(contact))
        .take(1)
    {
        if found.keys().next() == Some(&birthday) {
            println!("=====Found birthday:=====\n{}", birthday);
            found.insert(birthday, true);
        } else {
            println!("=====Adding birthday:=====\n{}", birthday);
            self.add_ics(birthday).await?;
        }
    }

The planned resource writer was:

    async fn add_ics(&self, ics: ICS) -> Result<(), CaldavError> {
        let id = ics.id().unwrap();
        let url = format!("{}{}", self.calendar_uri.path(), id);

        self.client
            .create_resource(&url, ics.0.into_bytes(), b"text/calendar")
            .await?;

        Ok(())
    }

The incomplete birthday calendar builder was:

    fn birthday_ical(contact: &Contact) -> Option<ICalendar> {
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

        // Convert `ics` into `ICalendar` once serialization/parsing supports
        // generated calendar resources, then return it here.
    }
*/
