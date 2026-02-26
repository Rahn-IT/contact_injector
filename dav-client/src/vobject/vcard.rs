use std::fmt::Display;

use nom::Input;

use crate::vobject::{VObject, VParseError};

#[derive(Debug)]
pub struct VCard {
    pub uid: Option<String>,
    pub org: Option<String>,
    pub tel: Vec<(TelType, PhoneNumber)>,
}

#[derive(Debug)]
pub struct PhoneNumber(String);

#[derive(Debug)]
pub enum TelType {
    Home,
    Work,
}

impl PhoneNumber {
    pub fn sanitize(number: &str) -> Option<Self> {
        if number.is_empty() {
            return None;
        }
        // replace 00 at start with + and keep existing +
        let mut iter = number.iter_elements();
        let mut sanitized = String::with_capacity(number.len());

        let first = iter.next().expect("number is not empty");
        if first == '0' {
            if let Some(second) = iter.next() {
                if second == '0' {
                    sanitized.push('+');
                } else {
                    sanitized.push(first);
                    sanitized.push(second);
                }
            }
        } else if first == '+' || first.is_numeric() {
            sanitized.push('0');
        }

        sanitized.extend(iter.filter(|c| c.is_numeric()));

        Some(Self(sanitized))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VCardError {
    #[error("error during syntax parsing: {0}")]
    VParseError(#[from] VParseError),
    #[error("missing VERSION property on ICalendar")]
    MissingVersion,
    #[error("unsupported VERSION property on ICalendar")]
    UnsupportedVersion,
}

impl VCard {
    pub fn parse(input: &str) -> Result<Self, VCardError> {
        let vobject = VObject::parse(input)?;

        let version = vobject
            .get_property_value("VERSION")
            .ok_or(VCardError::MissingVersion)?;

        if version != "3.0" {
            return Err(VCardError::UnsupportedVersion);
        }

        let uid = vobject.get_property_value("UID").map(|uid| uid.to_string());

        let tel = vobject
            .get_multi_property("TEL")
            .into_iter()
            .filter_map(|tel| {
                let tel_type = match tel.metadata.get("TYPE")?.to_lowercase().as_str() {
                    "home" => TelType::Home,
                    "work" => TelType::Work,
                    _ => TelType::Home,
                };

                let number = tel.values.first()?.as_str();
                let number = PhoneNumber::sanitize(number)?;

                Some((tel_type, number))
            })
            .collect::<Vec<_>>();

        let org = vobject.get_property_value("ORG").map(|org| org.to_string());

        Ok(Self { uid, org, tel })
    }
}
