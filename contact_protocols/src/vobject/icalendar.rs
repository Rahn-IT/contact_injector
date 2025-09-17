use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};

use crate::vobject::{VObject, VParseError, VProperty};

pub struct ICalendar {
    prodid: String,
    events: Vec<VEvent>,
}

pub struct VEvent {
    uid: String,
    summary: String,
    description: Option<String>,
    location: Option<String>,
    start: DateOrTime,
    end: DateOrTime,
    repeat: Option<Repeat>,
    created: DateTime<Utc>,
    stamp: DateTime<Utc>,
    modified: DateTime<Utc>,
    alarms: Vec<VAlarm>,
}

pub struct VAlarm {
    summary: String,
    description: Option<String>,
    action: Action,
}

#[derive(Debug, thiserror::Error)]
pub enum ICalError {
    #[error("error during syntax parsing: {0}")]
    VParseError(#[from] VParseError),
    #[error("missing PRODID property on ICalendar")]
    MissingProdid,
    #[error("missing VERSION property on ICalendar")]
    MissingVersion,
    #[error("unsupported VERSION property on ICalendar")]
    UnsupportedVersion,
    #[error("missing UID property on event")]
    MissingEventUid,
    #[error("missing SUMMARY property on event")]
    MissingEventSummary,
    #[error("missing DTSTART property on event")]
    MissingEventStart,
    #[error("missing DTEND property on event")]
    MissingEventEnd,
    #[error("unsupported date or time format")]
    UnsupportedDateOrTimeFormat,
    #[error("error parsing date or time")]
    DateTimeParseError(#[from] chrono::ParseError),
    #[error("missing CREATED property on event")]
    MissingEventCreated,
    #[error("missing LAST-MODIFIED property on event")]
    MissingEventLastModified,
    #[error("missing DTSTAMP property on event")]
    MissingEventStamp,
    #[error("unknown repeat type: {0}")]
    UnknownRepeatType(String),
    #[error("missing SUMMARY property on alarm")]
    MissingAlarmSummary,
    #[error("missing ACTION property on alarm")]
    MissingAlarmAction,
    #[error("unsupported ACTION property on alarm")]
    UnsupportedAlarmAction,
}

impl ICalendar {
    fn parse(input: &str) -> Result<Self, ICalError> {
        let vobject = VObject::parse(input)?;
        let prodid = vobject
            .get_property_value("PRODID")
            .ok_or(ICalError::MissingProdid)?
            .to_string();

        let version = vobject
            .get_property_value("VERSION")
            .ok_or(ICalError::MissingVersion)?;

        if version != "2.0" {
            return Err(ICalError::UnsupportedVersion);
        }

        let events: Result<Vec<VEvent>, ICalError> = vobject
            .sub_objects
            .into_iter()
            .filter(|event| event.class == "VEVENT")
            .map(VEvent::from_vobject)
            .collect();
        let events = events?;

        Ok(Self { prodid, events })
    }
}

impl VEvent {
    fn from_vobject(vobject: VObject) -> Result<Self, ICalError> {
        let uid = vobject
            .get_property_value("UID")
            .ok_or(ICalError::MissingEventUid)?
            .to_string();

        let summary = vobject
            .get_property_value("SUMMARY")
            .ok_or(ICalError::MissingEventSummary)?
            .to_string();

        let description = vobject
            .get_property_value("DESCRIPTION")
            .map(|s| s.to_string());

        let location = vobject
            .get_property_value("LOCATION")
            .map(|s| s.to_string());

        let start_prop = vobject
            .get_property("DTSTART")
            .ok_or(ICalError::MissingEventStart)?;

        let start = DateOrTime::from_vproperty(start_prop)?;

        let end_prop = vobject
            .get_property("DTEND")
            .ok_or(ICalError::MissingEventEnd)?;

        let end = DateOrTime::from_vproperty(end_prop)?;

        let repeat_raw = vobject.get_property_value("RRULE");

        let repeat = if let Some(repeat) = repeat_raw {
            Some(match repeat {
                "FREQ=YEARLY" => Repeat::Yearly,
                _ => return Err(ICalError::UnknownRepeatType(repeat.to_string())),
            })
        } else {
            None
        };

        let created_raw = vobject
            .get_property_value("CREATED")
            .ok_or(ICalError::MissingEventCreated)?;

        let created = DateTime::parse_from_str(created_raw, "%Y%m%dT%H%M%SZ")?.to_utc();

        let modified_raw = vobject
            .get_property_value("LAST-MODIFIED")
            .ok_or(ICalError::MissingEventLastModified)?;

        let modified = DateTime::parse_from_str(modified_raw, "%Y%m%dT%H%M%SZ")?.to_utc();

        let stamp_raw = vobject
            .get_property_value("DTSTAMP")
            .ok_or(ICalError::MissingEventStamp)?;

        let stamp = DateTime::parse_from_str(stamp_raw, "%Y%m%dT%H%M%SZ")?.to_utc();

        let alarms: Result<Vec<VAlarm>, ICalError> = vobject
            .sub_objects
            .into_iter()
            .filter(|alarm| alarm.class == "VALARM")
            .map(VAlarm::from_vobject)
            .collect();
        let alarms = alarms?;

        Ok(Self {
            uid,
            summary,
            description,
            location,
            start,
            end,
            repeat,
            created,
            modified,
            stamp,
            alarms,
        })
    }
}

impl VAlarm {
    fn from_vobject(vobject: VObject) -> Result<Self, ICalError> {
        let summary = vobject
            .get_property_value("SUMMARY")
            .ok_or(ICalError::MissingAlarmSummary)?
            .to_string();

        let description = vobject
            .get_property_value("DESCRIPTION")
            .map(|s| s.to_string());

        let action = vobject
            .get_property_value("ACTION")
            .ok_or(ICalError::MissingAlarmAction)?;

        let action = match action {
            "DISPLAY" => Action::Display,
            _ => return Err(ICalError::UnsupportedAlarmAction),
        };

        Ok(Self {
            summary,
            description,
            action,
        })
    }
}

pub enum Action {
    Display,
}

pub enum DateOrTime {
    Date(NaiveDate),
    DateTime(NaiveDateTime),
}

impl DateOrTime {
    fn from_vproperty(property: &VProperty) -> Result<Self, ICalError> {
        let property_value = property
            .values
            .first()
            .ok_or(ICalError::MissingEventStart)?
            .as_str();

        let parsed = match property.metadata.get("VALUE").map(|s| s.as_str()) {
            Some("DATE") => DateOrTime::Date(NaiveDate::parse_from_str(property_value, "%Y%m%d")?),
            None => DateOrTime::DateTime(NaiveDateTime::parse_from_str(
                property_value,
                "%y%m%dT%H%M%S",
            )?),
            _ => return Err(ICalError::UnsupportedDateOrTimeFormat),
        };

        Ok(parsed)
    }
}

pub enum Transparency {
    Transparent,
    Opaque,
}

impl Transparency {
    pub fn from_str(value: &str) -> Option<Transparency> {
        match value {
            "TRANSPARENT" => Some(Transparency::Transparent),
            "OPAQUE" => Some(Transparency::Opaque),
            _ => None,
        }
    }
}

pub enum Repeat {
    Yearly,
}
