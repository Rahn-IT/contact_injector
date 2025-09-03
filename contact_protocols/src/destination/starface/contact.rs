use std::{fmt::Debug, hash::Hash};

use nom::Input;

use crate::{
    contact::{Contact, PhoneType},
    destination::starface::{
        Attribute, Block, PhoneSchema, PhoneSchemaType, RawStarfaceContact, SummarySchema,
        SummarySchemaType, Tag, UploadContact,
    },
};

#[derive(Default, Eq)]
pub struct StarfaceContact {
    pub id: String,
    first_name: String,
    family_name: String,
    company: String,
    phone: String,
    mobile: String,
    home_phone: String,
}

impl Debug for StarfaceContact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StarfaceContact")
            .field("first_name", &self.first_name)
            .field("family_name", &self.family_name)
            .field("company", &self.company)
            .field("phone", &self.phone)
            .field("mobile", &self.mobile)
            .field("home_phone", &self.home_phone)
            .finish()
    }
}

impl Hash for StarfaceContact {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.first_name.hash(state);
        self.family_name.hash(state);
        self.company.hash(state);
        self.phone.hash(state);
        self.mobile.hash(state);
        self.home_phone.hash(state);
    }
}

impl PartialEq for StarfaceContact {
    fn eq(&self, other: &Self) -> bool {
        self.first_name == other.first_name
            && self.family_name == other.family_name
            && self.company == other.company
            && self.phone == other.phone
            && self.mobile == other.mobile
            && self.home_phone == other.home_phone
    }
}

impl StarfaceContact {
    pub fn from_raw(
        raw: RawStarfaceContact,
        summary_schema: &SummarySchema,
        phone_schema: &PhoneSchema,
    ) -> Self {
        let mut contact = StarfaceContact {
            id: raw.id,
            ..Default::default()
        };

        for (index, name) in raw.summary_values.into_iter().enumerate() {
            match summary_schema.attributes[index].name {
                SummarySchemaType::FirstName => {
                    contact.first_name = name;
                }
                SummarySchemaType::FamilyName => {
                    contact.family_name = name;
                }
                SummarySchemaType::Company => {
                    contact.company = name;
                }
            }
        }

        for (index, phone) in raw.phone_numbers.into_iter().enumerate() {
            match phone_schema.attributes[index].name {
                PhoneSchemaType::Phone => {
                    contact.phone = phone;
                }
                PhoneSchemaType::Mobile => {
                    contact.mobile = phone;
                }
                PhoneSchemaType::HomePhone => {
                    contact.home_phone = phone;
                }
            }
        }

        contact
    }

    pub fn from_contact(contact: &Contact) -> Option<Self> {
        let first_name = sanitize_name(contact.name.given_name.as_ref());
        let family_name = sanitize_name(contact.name.family_name.as_ref());
        let company = sanitize_name(contact.org.as_ref());

        if first_name.is_empty() && family_name.is_empty() && company.is_empty() {
            return None;
        }

        let phone = sanitize_number(
            contact
                .phones
                .iter()
                .find(|phone| phone.phone_type == PhoneType::Work)
                .map(|phone| &phone.number),
        );
        let home_phone = sanitize_number(
            contact
                .phones
                .iter()
                .find(|phone| phone.phone_type == PhoneType::Home)
                .map(|phone| &phone.number),
        );
        let mobile = sanitize_number(
            contact
                .phones
                .iter()
                .find(|phone| phone.phone_type == PhoneType::Mobile)
                .map(|phone| &phone.number),
        );

        Some(Self {
            id: String::new(),
            first_name,
            family_name,
            company,
            phone,
            home_phone,
            mobile,
        })
    }

    pub fn to_upload<'a>(&'a self, tag: &'a Tag) -> UploadContact<'a> {
        UploadContact {
            blocks: vec![
                Block {
                    name: "contact",
                    attributes: vec![
                        Attribute {
                            name: "firstname",
                            value: &self.first_name,
                        },
                        Attribute {
                            name: "familyname",
                            value: &self.family_name,
                        },
                        Attribute {
                            name: "company",
                            value: &self.company,
                        },
                    ],
                },
                Block {
                    name: "telephone",
                    attributes: vec![
                        Attribute {
                            name: "phone",
                            value: &self.phone,
                        },
                        Attribute {
                            name: "homephone",
                            value: &self.home_phone,
                        },
                        Attribute {
                            name: "mobile",
                            value: &self.mobile,
                        },
                        Attribute {
                            name: "fax",
                            value: "",
                        },
                    ],
                },
            ],
            id: "",
            editable: true,
            tags: vec![tag],
        }
    }
}

fn sanitize_number(number: Option<&String>) -> String {
    let number: String = if let Some(number) = number {
        if number.is_empty() {
            return String::new();
        } else {
            number
                .as_str()
                .iter_elements()
                .filter(|c| c.is_numeric() || c == &'+')
                .collect()
        }
    } else {
        return String::new();
    };

    let mut sanitized = String::new();

    // replace 00 at start with + and keep existing +
    let mut iter = number.as_str().iter_elements();
    let first = iter.next().unwrap();
    match first {
        '+' => {
            sanitized = number;
        }
        '0' => {
            if iter.next() == Some('0') {
                sanitized.push('+');
                sanitized.push_str(&number[2..].replace('+', ""));
            } else {
                sanitized.push_str("+49");
                sanitized.push_str(&number[1..].replace('+', ""));
            }
        }
        '1'..'9' => {
            sanitized.push_str("+49");
            sanitized.push_str(&number.replace('+', ""));
        }
        _ => {
            unreachable!();
        }
    };

    sanitized
}

fn sanitize_name(name: Option<&String>) -> String {
    let name = if let Some(name) = name {
        if name.is_empty() {
            return String::new();
        } else {
            name
        }
    } else {
        return String::new();
    };

    name.trim().to_string()
}
