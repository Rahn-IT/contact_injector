//! Contact models and adapters for address books, phone systems, and calendars.
pub mod contact;
mod destination;
mod source;

pub use destination::ContactDestination;
pub use destination::{caldav_birthdays, starface};
pub use source::ContactSource;
pub use source::carddav;
