use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};

pub struct ICalendar {
    prodid: String,
}

pub struct VEvent {
    uid: String,
    summary: String,
    start: DateOrTime,
    end: DateOrTime,
    created: DateTime<Utc>,
    stamp: DateTime<Utc>,
    last_modified: DateTime<Utc>,
    alarms: Vec<VAlarm>,
}

pub struct VAlarm {
    summary: String,
    //Todo trigger
    description: String,
    action: Action,
}

pub enum Action {
    Display,
}

pub enum DateOrTime {
    Date(NaiveDate),
    DateTime(NaiveDateTime),
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
