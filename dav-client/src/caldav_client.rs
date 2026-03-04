use reqwest::Url;

use crate::{
    dav_client::{DavClient, DavError, ResourceRef},
    vobject::{
        icalendar::{ICalError, ICalendar},
        vcard::{VCard, VCardError},
    },
};

/// ---------- Error handling ----------

#[derive(Debug, thiserror::Error)]
pub enum CalDavError {
    #[error("dav error: {0}")]
    DavError(#[from] DavError),
    #[error("error parsing icalendar: {0}")]
    ParseError(#[from] ICalError),
}

/// ---------- Data types ----------

#[derive(Debug, Clone)]
pub struct ICalRef(ResourceRef);

#[derive(Clone)]
pub struct CalDavClient {
    client: DavClient,
}

const CALENDAR_QUERY: &str = "c:calendar-query";
const CALENDAR_MULTIGET: &str = "c:calendar-multiget";
const CALDAV_NAMESPACE: &str = "urn:ietf:params:xml:ns:caldav";
const CALDAV_DATA_NAMESPACE: &str = "c:calendar-data";

/// ---------- Client ----------

impl CalDavClient {
    pub fn new(addressbook_url: Url, username: &str, password: &str) -> Result<Self, CalDavError> {
        Ok(Self {
            client: DavClient::new(addressbook_url, username, password)?,
        })
    }

    /// ---------- List contacts ----------
    pub async fn list_calendar_entries(&self) -> Result<Vec<ICalRef>, CalDavError> {
        let refs = self
            .client
            .list_resources(CALENDAR_QUERY, CALDAV_NAMESPACE)
            .await?
            .into_iter()
            .map(ICalRef)
            .collect();

        Ok(refs)
    }

    pub async fn fetch_calendar_entries(
        &self,
        contacts: &[ICalRef],
    ) -> Result<Vec<ICalendar>, CalDavError> {
        let refs = contacts
            .iter()
            .map(|contact| contact.0.clone())
            .collect::<Vec<_>>();

        let resources = self
            .client
            .multiget(
                &refs,
                CALENDAR_MULTIGET,
                CALDAV_NAMESPACE,
                CALDAV_DATA_NAMESPACE,
            )
            .await?;

        let icals = resources
            .iter()
            .map(|resource| ICalendar::parse(resource))
            .collect::<Result<_, ICalError>>()?;

        Ok(icals)
    }
}
