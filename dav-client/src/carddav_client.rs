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

        eprintln!(
            "[carddav] parsing {} vCards returned for {} requested resources",
            resources.len(),
            refs.len()
        );

        let contacts = resources
            .iter()
            .enumerate()
            .map(|(index, resource)| {
                VCard::parse(resource).map_err(|error| {
                    let requested_href = refs
                        .get(index)
                        .map(|resource| resource.href.as_str())
                        .unwrap_or("<no matching requested resource>");
                    let last_non_empty_line = resource
                        .lines()
                        .rev()
                        .find(|line| !line.trim().is_empty())
                        .unwrap_or("<empty vCard>");

                    eprintln!("[carddav] failed to parse vCard at batch index {index}");
                    eprintln!("[carddav] requested href at this index: {requested_href}");
                    eprintln!("[carddav] parser error: {error}");
                    eprintln!(
                        "[carddav] payload: {} bytes, {} lines, begins with BEGIN:VCARD={}, ends with END:VCARD={}, last non-empty line={last_non_empty_line:?}",
                        resource.len(),
                        resource.lines().count(),
                        resource.trim_start().starts_with("BEGIN:VCARD"),
                        resource.trim_end().ends_with("END:VCARD"),
                    );
                    eprintln!("[carddav] raw failing vCard follows:\n{resource}");

                    error
                })
            })
            .collect::<Result<_, VCardError>>()?;

        Ok(contacts)
    }
}
