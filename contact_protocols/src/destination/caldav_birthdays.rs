use crate::contact::Contact;
use chrono::{Days, Utc};
use dav_client::caldav_client::CalDavClient;
use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use crate::ContactDestination;

#[derive(Serialize, Deserialize)]
pub struct CaldavAccessData {
    pub url: String,
    pub username: String,
    pub password: String,
}

pub struct CaldavBirthdayDestination {
    client: CalDavClient,
}

const UID_PREFIX: &str = "contact-injector-birthday-v1-";
const UID_DOMAIN: &str = "rahn-it.de";
const SOFTWARE_MARKER: &str = "BDAY-V1";

#[derive(Debug, thiserror::Error)]
pub enum CaldavError {
    #[error("inner error: {0}")]
    Inner(#[from] dav_client::caldav_client::CalDavError),
    #[error("invalid addressbook URI: {0:?}")]
    InvalidAddressbookUri(#[from] url::ParseError),
    #[error("refusing to sync an empty contact list to the birthday calendar")]
    EmptyContactList,
    #[error("birthday date cannot be advanced by one day")]
    BirthdayDateOverflow,
}

impl CaldavBirthdayDestination {
    pub async fn new(access_data: CaldavAccessData) -> Result<Self, CaldavError> {
        let calendar_url: Url = access_data.url.parse()?;
        let username = access_data.username;
        let password = access_data.password;

        let client = CalDavClient::new(calendar_url.clone(), &username, &password)?;

        client.list_events_by_uid_prefix(UID_PREFIX).await?;

        Ok(Self { client })
    }
}

impl ContactDestination for CaldavBirthdayDestination {
    type Error = CaldavError;

    async fn export_contacts(
        &self,
        contacts: impl Iterator<Item = &Contact>,
    ) -> Result<(), Self::Error> {
        let mut contacts = contacts.peekable();
        if contacts.peek().is_none() {
            return Err(CaldavError::EmptyContactList);
        }

        println!("Exporting birthdays to CalDAV");
        let existing = self.client.list_events_by_uid_prefix(UID_PREFIX).await?;
        let birthdays = contacts
            .filter_map(birthday_calendar_entry)
            .collect::<Result<Vec<_>, _>>()?;

        println!(
            "Found {} existing managed entries; creating {} birthday entries",
            existing.len(),
            birthdays.len()
        );
        let birthday_count = birthdays.len();

        // Create everything first. If one creation fails, the existing working
        // calendar remains intact and the next run will clean up the partial set.
        for birthday in birthdays {
            self.client
                .create_calendar_entry(&birthday.resource_name, birthday.data)
                .await?;
        }

        for entry in &existing {
            self.client.delete_calendar_entry(entry).await?;
        }

        println!(
            "Birthday calendar sync complete: deleted {}, created {}",
            existing.len(),
            birthday_count
        );

        Ok(())
    }
}

struct CalendarEntry {
    resource_name: String,
    data: String,
}

fn birthday_calendar_entry(contact: &Contact) -> Option<Result<CalendarEntry, CaldavError>> {
    let birthday = contact.birthday?;
    let end = match birthday.checked_add_days(Days::new(1)) {
        Some(end) => end,
        None => return Some(Err(CaldavError::BirthdayDateOverflow)),
    };
    let id = Uuid::new_v4();
    let uid = format!("{UID_PREFIX}{id}@{UID_DOMAIN}");
    let resource_name = format!("{UID_PREFIX}{id}.ics");
    let now = Utc::now().format("%Y%m%dT%H%M%SZ");
    let summary = escape_ical_text(&format!("Birthday: {}", contact.display_name));

    let lines = [
        "BEGIN:VCALENDAR".to_string(),
        "PRODID:-//Rahn IT//Contact Injector//EN".to_string(),
        "VERSION:2.0".to_string(),
        "CALSCALE:GREGORIAN".to_string(),
        "BEGIN:VEVENT".to_string(),
        format!("UID:{uid}"),
        format!("DTSTAMP:{now}"),
        format!("DTSTART;VALUE=DATE:{}", birthday.format("%Y%m%d")),
        format!("DTEND;VALUE=DATE:{}", end.format("%Y%m%d")),
        "RRULE:FREQ=YEARLY".to_string(),
        format!("SUMMARY:{summary}"),
        "TRANSP:TRANSPARENT".to_string(),
        "CLASS:CONFIDENTIAL".to_string(),
        format!("X-RAHNIT-CONTACT-INJECTOR:{SOFTWARE_MARKER}"),
        "BEGIN:VALARM".to_string(),
        "TRIGGER;VALUE=DURATION:-P0D".to_string(),
        "ACTION:DISPLAY".to_string(),
        format!("DESCRIPTION:{summary}"),
        "END:VALARM".to_string(),
        "END:VEVENT".to_string(),
        "END:VCALENDAR".to_string(),
    ];

    let mut data = String::new();
    for line in lines {
        data.push_str(&fold_ical_line(&line));
        data.push_str("\r\n");
    }

    Some(Ok(CalendarEntry {
        resource_name,
        data,
    }))
}

fn escape_ical_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace("\r\n", "\\n")
        .replace(['\r', '\n'], "\\n")
}

fn fold_ical_line(line: &str) -> String {
    let mut folded = String::new();
    let mut octets = 0;

    for character in line.chars() {
        let character_octets = character.len_utf8();
        if octets + character_octets > 75 {
            folded.push_str("\r\n ");
            octets = 1;
        }
        folded.push(character);
        octets += character_octets;
    }

    folded
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    #[test]
    fn creates_marked_recurring_birthday_event() {
        let contact = Contact {
            display_name: "Doe, Jane; R&D".to_string(),
            birthday: Some(NaiveDate::from_ymd_opt(1990, 5, 6).unwrap()),
            ..Default::default()
        };

        let entry = birthday_calendar_entry(&contact)
            .expect("contact has a birthday")
            .expect("birthday should serialize");

        assert!(entry.resource_name.starts_with(UID_PREFIX));
        assert!(entry.resource_name.ends_with(".ics"));
        assert!(entry.data.contains(&format!("UID:{UID_PREFIX}")));
        assert!(entry.data.contains("X-RAHNIT-CONTACT-INJECTOR:BDAY-V1\r\n"));
        assert!(entry.data.contains("DTSTART;VALUE=DATE:19900506\r\n"));
        assert!(entry.data.contains("DTEND;VALUE=DATE:19900507\r\n"));
        assert!(entry.data.contains("RRULE:FREQ=YEARLY\r\n"));
        assert!(entry.data.contains("BEGIN:VALARM\r\n"));
        assert!(entry.data.contains("ACTION:DISPLAY\r\n"));
        assert!(
            entry
                .data
                .contains("SUMMARY:Birthday: Doe\\, Jane\\; R&D\r\n")
        );
        assert!(entry.data.ends_with("END:VCALENDAR\r\n"));
    }

    #[test]
    fn folds_lines_without_splitting_utf8_characters() {
        let folded = fold_ical_line(&format!("SUMMARY:{}", "Geburtstag 🎂 ".repeat(10)));

        assert!(folded.contains("\r\n "));
        assert!(folded.split("\r\n").all(|line| line.len() <= 75));
    }
}
