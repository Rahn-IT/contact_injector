use reqwest::Url;

use crate::{
    dav_client::{DavClient, DavError, ResourceRef},
    vobject::vcard::{VCard, VCardError},
};

/// ---------- Error handling ----------

#[derive(Debug, thiserror::Error)]
pub enum CardDavError {
    #[error("dav error: {0}")]
    DavError(#[from] DavError),
    #[error("error parsing vcard: {0}")]
    ParseError(#[from] VCardError),
}

/// ---------- Data types ----------

#[derive(Debug, Clone)]
pub struct ContactRef(ResourceRef);

#[derive(Clone)]
pub struct CardDavClient {
    client: DavClient,
}

const ADDRESSBOOK_QUERY: &str = "c:addressbook-query";
const ADDRESSBOOK_MULTIGET: &str = "c:addressbook-multiget";
const CARDDAV_NAMESPACE: &str = "urn:ietf:params:xml:ns:carddav";
const CARDDAV_DATA_NAMESPACE: &str = "c:address-data";

/// ---------- Client ----------

impl CardDavClient {
    pub fn new(addressbook_url: Url, username: &str, password: &str) -> Result<Self, CardDavError> {
        Ok(Self {
            client: DavClient::new(addressbook_url, username, password)?,
        })
    }

    /// ---------- List contacts ----------
    pub async fn list_contacts(&self) -> Result<Vec<ContactRef>, CardDavError> {
        let refs = self
            .client
            .list_resources(ADDRESSBOOK_QUERY, CARDDAV_NAMESPACE)
            .await?
            .into_iter()
            .map(ContactRef)
            .collect();

        Ok(refs)
    }

    pub async fn fetch_contacts(
        &self,
        contacts: &[ContactRef],
    ) -> Result<Vec<VCard>, CardDavError> {
        let refs = contacts
            .iter()
            .map(|contact| contact.0.clone())
            .collect::<Vec<_>>();

        let resources = self
            .client
            .multiget(
                &refs,
                ADDRESSBOOK_MULTIGET,
                CARDDAV_NAMESPACE,
                CARDDAV_DATA_NAMESPACE,
            )
            .await?;

        let contacts = resources
            .iter()
            .map(|resource| VCard::parse(resource))
            .collect::<Result<_, VCardError>>()?;

        Ok(contacts)
    }
}
