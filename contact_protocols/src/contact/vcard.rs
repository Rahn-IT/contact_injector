use dav_client::vobject::vcard::VCard;

use crate::contact::{
    Address, AddressType, Attachment, Contact, ContactPhoto, Email, EmailType, Name, Phone,
    PhoneType, Url, UrlType,
};

#[derive(Debug, thiserror::Error)]
pub enum FromVCardError {
    #[error("No display name in vcard")]
    MissingDisplayName,
}

impl Contact {
    pub fn from_vcard(vcard: &VCard) -> Contact {
        Contact {
            display_name: vcard.display_name.clone(),
            name: Name {
                family_name: vcard.name.family_name.clone(),
                given_name: vcard.name.given_name.clone(),
                additional_names: vcard.name.additional_names.clone(),
                honorific_prefixes: vcard.name.honorific_prefixes.clone(),
                honorific_suffixes: vcard.name.honorific_suffixes.clone(),
            },
            nickname: vcard.nickname.clone(),
            emails: vcard
                .emails
                .iter()
                .map(|email| Email {
                    email: email.email.clone(),
                    email_type: match email.email_type {
                        dav_client::vobject::vcard::EmailType::Home => EmailType::Home,
                        dav_client::vobject::vcard::EmailType::Work => EmailType::Work,
                    },
                })
                .collect(),
            phones: vcard
                .phones
                .iter()
                .map(|phone| Phone {
                    number: phone.number.clone(),
                    phone_type: match phone.phone_type {
                        dav_client::vobject::vcard::PhoneType::Home => PhoneType::Home,
                        dav_client::vobject::vcard::PhoneType::Mobile => PhoneType::Mobile,
                        dav_client::vobject::vcard::PhoneType::Work => PhoneType::Work,
                        dav_client::vobject::vcard::PhoneType::Fax => PhoneType::Fax,
                        dav_client::vobject::vcard::PhoneType::Secr => PhoneType::Secr,
                        dav_client::vobject::vcard::PhoneType::Pager => PhoneType::Pager,
                        dav_client::vobject::vcard::PhoneType::Car => PhoneType::Car,
                        dav_client::vobject::vcard::PhoneType::Other => PhoneType::Other,
                    },
                })
                .collect(),
            addresses: vcard
                .addresses
                .iter()
                .map(|address| Address {
                    address_type: match address.address_type {
                        dav_client::vobject::vcard::AddressType::Home => AddressType::Home,
                        dav_client::vobject::vcard::AddressType::Work => AddressType::Work,
                        dav_client::vobject::vcard::AddressType::Other => AddressType::Other,
                    },
                    post_box: address.post_box.clone(),
                    extension: address.extension.clone(),
                    street: address.street.clone(),
                    locality: address.locality.clone(),
                    region: address.region.clone(),
                    postal_code: address.postal_code.clone(),
                    country: address.country.clone(),
                })
                .collect(),
            birthday: vcard.birthday,
            anniversary: vcard.anniversary,
            photo: vcard.photo.as_ref().map(|photo| ContactPhoto {
                data: photo.data.clone(),
            }),
            revision: vcard.revision,
            uid: vcard.uid.clone(),
            org: vcard.org.clone(),
            title: vcard.title.clone(),
            urls: vcard
                .urls
                .iter()
                .map(|url| Url {
                    url: url.url.clone(),
                    url_type: match url.url_type {
                        dav_client::vobject::vcard::UrlType::Work => UrlType::Work,
                    },
                })
                .collect(),
            note: vcard.note.clone(),
            attachments: vcard
                .attachments
                .iter()
                .map(|attachment| Attachment {
                    data: attachment.data.clone(),
                })
                .collect(),
        }
    }
}
