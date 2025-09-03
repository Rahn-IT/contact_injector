pub mod contact;
mod destination;
mod source;
mod vcard;

pub use destination::ContactDestination;
pub use destination::starface;
pub use source::ContactSource;
pub use source::carddav;
pub use vcard::parse_vcard;
