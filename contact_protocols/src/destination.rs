use crate::contact::Contact;

pub mod starface;

pub trait ContactDestination {
    type Error: std::error::Error;

    fn export_contacts<'a>(
        &self,
        contacts: impl Iterator<Item = &'a Contact>,
    ) -> impl Future<Output = Result<(), Self::Error>>;
}
