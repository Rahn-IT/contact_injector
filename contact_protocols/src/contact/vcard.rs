use dav_client::vobject::vcard::VCard;

use crate::contact::Contact;

#[derive(Debug, thiserror::Error)]
pub enum FromVCardError {
    #[error("No display name in vcard")]
    MissingDisplayName,
}

impl Contact {
    pub fn from_vcard(vcard: &VCard) -> Contact {
        todo!()
    }
}
