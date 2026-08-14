use reqwest::Url;

use crate::{
    dav_client::{DavClient, DavError, ResourceRef},
    vobject::icalendar::{ICalError, ICalendar},
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

impl ICalRef {
    pub fn resource_name(&self) -> Option<&str> {
        self.0.href.trim_end_matches('/').rsplit('/').next()
    }
}

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

    pub async fn list_events_by_uid_prefix(
        &self,
        uid_prefix: &str,
    ) -> Result<Vec<ICalRef>, CalDavError> {
        let refs = self
            .client
            .list_resources_by_property(
                CALENDAR_QUERY,
                CALDAV_NAMESPACE,
                "VEVENT",
                "UID",
                uid_prefix,
            )
            .await?
            .into_iter()
            .map(ICalRef)
            .collect();

        Ok(refs)
    }

    pub async fn create_calendar_entry(
        &self,
        resource_name: &str,
        calendar_data: String,
    ) -> Result<(), CalDavError> {
        self.client
            .create_resource(resource_name, calendar_data, "text/calendar; charset=utf-8")
            .await?;
        Ok(())
    }

    pub async fn delete_calendar_entry(&self, entry: &ICalRef) -> Result<(), CalDavError> {
        self.client.delete_resource(&entry.0.href).await?;
        Ok(())
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
