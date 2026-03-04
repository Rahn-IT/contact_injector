use crate::vobject::{VObject, VParseError};

#[derive(Debug, Default)]
pub struct VCard {
    pub display_name: String,
    pub name: Name,
    pub nickname: Option<String>,
    pub emails: Vec<Email>,
    pub phones: Vec<Phone>,
    pub addresses: Vec<Address>,
    pub birthday: Option<chrono::NaiveDate>,
    pub anniversary: Option<chrono::NaiveDate>,
    pub photo: Option<ContactPhoto>,
    pub revision: Option<chrono::NaiveDateTime>,
    pub uid: Option<String>,
    pub org: Option<String>,
    pub title: Option<String>,
    pub urls: Vec<Url>,
    pub note: Option<String>,
    pub attachments: Vec<Attachment>,
}

#[derive(Debug, Default)]
pub struct Name {
    pub family_name: Option<String>,
    pub given_name: Option<String>,
    pub additional_names: Vec<String>,
    pub honorific_prefixes: Vec<String>,
    pub honorific_suffixes: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Phone {
    pub number: String,
    pub phone_type: PhoneType,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub enum PhoneType {
    #[default]
    Home,
    Mobile,
    Work,
    Fax,
    Secr,
    Pager,
    Car,
    Other,
}

impl PhoneType {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "home" => Some(PhoneType::Home),
            "cell" => Some(PhoneType::Mobile),
            "work" => Some(PhoneType::Work),
            "fax" => Some(PhoneType::Fax),
            "secr" => Some(PhoneType::Secr),
            "pager" => Some(PhoneType::Pager),
            "car" => Some(PhoneType::Car),
            "other" => Some(PhoneType::Other),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct Email {
    pub email: String,
    pub email_type: EmailType,
}

#[derive(Debug, Default)]
pub enum EmailType {
    #[default]
    Home,
    Work,
}

impl EmailType {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "home" => Some(EmailType::Home),
            "work" => Some(EmailType::Work),
            _ => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct Address {
    pub address_type: AddressType,
    pub post_box: Option<String>,
    pub extension: Option<String>,
    pub street: Option<String>,
    pub locality: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Default)]
pub enum AddressType {
    #[default]
    Home,
    Work,
    Other,
}

impl AddressType {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "home" => Some(AddressType::Home),
            "work" => Some(AddressType::Work),
            "other" => Some(AddressType::Other),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct ContactPhoto {
    pub data: Vec<u8>,
}

impl std::fmt::Debug for ContactPhoto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContactPhoto").finish()
    }
}

#[derive(Default)]
pub struct Attachment {
    pub data: Vec<u8>,
}

impl std::fmt::Debug for Attachment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Attachment").finish()
    }
}

#[derive(Debug, Default)]
pub struct Url {
    pub url: String,
    pub url_type: UrlType,
}

#[derive(Debug, Default)]
pub enum UrlType {
    #[default]
    Work,
}

impl UrlType {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "work" => Some(UrlType::Work),
            _ => None,
        }
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

        let display_name = vobject
            .get_property_value("FN")
            .map(parse_text)
            .unwrap_or_default();

        let name = vobject
            .get_property_value("N")
            .map(parse_name)
            .unwrap_or_default();

        let nickname = vobject.get_property_value("NICKNAME").map(parse_text);

        let phones = vobject
            .get_multi_property("TEL")
            .into_iter()
            .filter_map(|tel| {
                let types = property_types(tel);
                let mut phone_type = PhoneType::Home;
                for t in types {
                    if let Some(parsed) = PhoneType::from_str(t) {
                        phone_type = parsed;
                        if phone_type == PhoneType::Fax {
                            break;
                        }
                    }
                }

                let number = parse_text(tel.values.first()?.as_str());
                if number.is_empty() {
                    return None;
                }

                Some(Phone { number, phone_type })
            })
            .collect::<Vec<_>>();

        let emails = vobject
            .get_multi_property("EMAIL")
            .into_iter()
            .filter_map(|email| {
                let value = parse_text(email.values.first()?.as_str());
                if value.is_empty() {
                    return None;
                }

                let mut email_type = EmailType::Home;
                for t in property_types(email) {
                    if let Some(parsed) = EmailType::from_str(t) {
                        email_type = parsed;
                        break;
                    }
                }

                Some(Email {
                    email: value,
                    email_type,
                })
            })
            .collect::<Vec<_>>();

        let addresses = vobject
            .get_multi_property("ADR")
            .into_iter()
            .map(parse_address)
            .collect::<Vec<_>>();

        let birthday = vobject
            .get_property_value("BDAY")
            .and_then(parse_date_or_datetime);

        let anniversary = vobject
            .get_property_value("ANNIVERSARY")
            .and_then(parse_date_or_datetime);

        let photo = vobject
            .get_property_value("PHOTO")
            .and_then(parse_binary)
            .map(|data| ContactPhoto { data });

        let revision = vobject.get_property_value("REV").and_then(parse_datetime);

        let uid = vobject.get_property_value("UID").map(parse_text);
        let org = vobject.get_property_value("ORG").map(|org| org.to_string());
        let title = vobject.get_property_value("TITLE").map(parse_text);

        let urls = vobject
            .get_multi_property("URL")
            .into_iter()
            .filter_map(|url| {
                let value = parse_text(url.values.first()?.as_str());
                if value.is_empty() {
                    return None;
                }

                let mut url_type = UrlType::Work;
                for t in property_types(url) {
                    if let Some(parsed) = UrlType::from_str(t) {
                        url_type = parsed;
                        break;
                    }
                }

                Some(Url {
                    url: value,
                    url_type,
                })
            })
            .collect::<Vec<_>>();

        let note = vobject.get_property_value("NOTE").map(parse_text);

        let attachments = vobject
            .get_multi_property("ATTACH")
            .into_iter()
            .filter_map(|attach| attach.values.first())
            .filter_map(|value| parse_binary(value))
            .map(|data| Attachment { data })
            .collect::<Vec<_>>();

        Ok(Self {
            display_name,
            name,
            nickname,
            emails,
            phones,
            addresses,
            birthday,
            anniversary,
            photo,
            revision,
            uid,
            org,
            title,
            urls,
            note,
            attachments,
        })
    }
}

fn parse_name(raw: &str) -> Name {
    let parts = raw.split(';').collect::<Vec<_>>();
    let parse_list = |index: usize| {
        parts
            .get(index)
            .map(|value| {
                value
                    .split(',')
                    .filter(|part| !part.trim().is_empty())
                    .map(parse_text)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };

    Name {
        family_name: parts
            .first()
            .map(|value| parse_text(value))
            .filter(|value| !value.is_empty()),
        given_name: parts
            .get(1)
            .map(|value| parse_text(value))
            .filter(|value| !value.is_empty()),
        additional_names: parse_list(2),
        honorific_prefixes: parse_list(3),
        honorific_suffixes: parse_list(4),
    }
}

fn parse_address(property: &crate::vobject::VProperty) -> Address {
    let mut address = Address::default();
    for t in property_types(property) {
        if let Some(parsed) = AddressType::from_str(t) {
            address.address_type = parsed;
            break;
        }
    }

    let raw = property.values.first().map(|s| s.as_str()).unwrap_or_default();
    let parts = raw.split(';').collect::<Vec<_>>();

    address.post_box = parts
        .first()
        .map(|s| parse_text(s))
        .filter(|s| !s.is_empty());
    address.extension = parts
        .get(1)
        .map(|s| parse_text(s))
        .filter(|s| !s.is_empty());
    address.street = parts
        .get(2)
        .map(|s| parse_text(s))
        .filter(|s| !s.is_empty());
    address.locality = parts
        .get(3)
        .map(|s| parse_text(s))
        .filter(|s| !s.is_empty());
    address.region = parts
        .get(4)
        .map(|s| parse_text(s))
        .filter(|s| !s.is_empty());
    address.postal_code = parts
        .get(5)
        .map(|s| parse_text(s))
        .filter(|s| !s.is_empty());
    address.country = parts
        .get(6)
        .map(|s| parse_text(s))
        .filter(|s| !s.is_empty());

    address
}

fn property_types<'a>(property: &'a crate::vobject::VProperty) -> impl Iterator<Item = &'a str> {
    property
        .metadata
        .get("TYPE")
        .map(|value| value.as_str())
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

fn parse_text(raw: &str) -> String {
    raw.replace("\\n", "\n")
        .replace("\\N", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}

fn parse_date_or_datetime(raw: &str) -> Option<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(raw, "%Y%m%d")
        .ok()
        .or_else(|| chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d").ok())
        .or_else(|| parse_datetime(raw).map(|dt| dt.date()))
}

fn parse_datetime(raw: &str) -> Option<chrono::NaiveDateTime> {
    chrono::NaiveDateTime::parse_from_str(raw, "%Y%m%dT%H%M%SZ")
        .ok()
        .or_else(|| chrono::NaiveDateTime::parse_from_str(raw, "%Y%m%dT%H%M%S").ok())
        .or_else(|| chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S").ok())
}

fn parse_binary(raw: &str) -> Option<Vec<u8>> {
    base64::Engine::decode(&base64::prelude::BASE64_STANDARD, raw).ok()
}
