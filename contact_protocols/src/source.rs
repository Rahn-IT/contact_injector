use crate::contact::Contact;

pub mod carddav;

pub trait ContactSource {
    type Error: std::error::Error;

    fn fetch_contacts(&self) -> impl Future<Output = Result<Vec<Contact>, Self::Error>> + Send;
}
