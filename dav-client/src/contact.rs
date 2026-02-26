use std::fmt::Debug;

#[derive(Debug, Default)]
pub struct Contact {
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

impl Contact {
    pub fn get_number(&self, phone_type: PhoneType) -> Option<String> {
        self.phones
            .iter()
            .find(|phone| phone.phone_type == phone_type)
            .map(|phone| phone.number.clone())
    }
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
    pub fn from_str(s: &str) -> Result<Self, ()> {
        match s.to_lowercase().as_str() {
            "home" => Ok(PhoneType::Home),
            "cell" => Ok(PhoneType::Mobile),
            "work" => Ok(PhoneType::Work),
            "fax" => Ok(PhoneType::Fax),
            "secr" => Ok(PhoneType::Secr),
            "pager" => Ok(PhoneType::Pager),
            "car" => Ok(PhoneType::Car),
            "other" => Ok(PhoneType::Other),
            "voice" | "pref" => Err(()),
            unknown => {
                eprintln!("Unknown phone type: {}", unknown);
                Err(())
            }
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
    pub fn from_str(s: &str) -> Result<Self, ()> {
        match s.to_lowercase().as_str() {
            "home" => Ok(EmailType::Home),
            "work" => Ok(EmailType::Work),
            "internet" | "pref" => Err(()),
            unknown => {
                eprintln!("Unknown email type: {}", unknown);
                Err(())
            }
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
    pub fn from_str(s: &str) -> Result<Self, ()> {
        match s.to_lowercase().as_str() {
            "work" => Ok(AddressType::Work),
            "home" => Ok(AddressType::Home),
            "other" => Ok(AddressType::Other),
            "pref" => Err(()),
            unknown => {
                eprintln!("Unknown address type: {}", unknown);
                Err(())
            }
        }
    }
}

#[derive(Default)]
pub struct ContactPhoto {
    pub data: Vec<u8>,
}

impl Debug for ContactPhoto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContactPhoto").finish()
    }
}

#[derive(Default)]
pub struct Attachment {
    pub data: Vec<u8>,
}

impl Debug for Attachment {
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
    pub fn from_str(s: &str) -> Result<Self, ()> {
        match s.to_lowercase().as_str() {
            "work" => Ok(UrlType::Work),
            "pref" => Err(()),
            unknown => {
                eprintln!("Unknown url type: {}", unknown);
                Err(())
            }
        }
    }
}
