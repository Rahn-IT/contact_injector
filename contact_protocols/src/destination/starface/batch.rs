//! A nonempty set of contacts that Starface can represent.

use super::{Error, contact};

/// Proof that conversion left at least one contact before destructive syncing.
pub(super) struct Batch(Vec<contact::Contact>);

impl Batch {
    pub(super) fn try_from_contacts<'a>(
        contacts: impl Iterator<Item = &'a crate::contact::Contact>,
    ) -> Result<Self, Error> {
        let contacts: Vec<_> = contacts
            .filter_map(contact::Contact::from_contact)
            .collect();
        if contacts.is_empty() {
            return Err(Error::EmptyContactList);
        }
        Ok(Self(contacts))
    }
}

impl IntoIterator for Batch {
    type Item = contact::Contact;
    type IntoIter = std::vec::IntoIter<contact::Contact>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::Batch;
    use crate::contact::{Contact, Name};

    #[test]
    fn rejects_nonempty_input_when_conversion_drops_every_contact() {
        let contacts = [Contact {
            display_name: "Display name only".into(),
            ..Contact::default()
        }];
        assert!(Batch::try_from_contacts(contacts.iter()).is_err());
    }

    #[test]
    fn keeps_exportable_contacts_in_a_mixed_source() {
        let contacts = [
            Contact::default(),
            Contact {
                name: Name {
                    given_name: Some("Ada".into()),
                    ..Name::default()
                },
                ..Contact::default()
            },
        ];
        assert_eq!(
            Batch::try_from_contacts(contacts.iter())
                .unwrap()
                .into_iter()
                .count(),
            1
        );
    }
}
