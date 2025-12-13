mod carddav_client;
pub mod contact;
mod destination;
mod source;
mod vcard;
mod vobject;

pub use destination::ContactDestination;
pub use destination::{caldav_birthdays, starface};
pub use source::ContactSource;
pub use source::carddav;
pub use vcard::parse_vcard;
