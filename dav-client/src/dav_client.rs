use std::io::Cursor;

use base64::{Engine, engine::general_purpose};
use quick_xml::events::{BytesDecl, BytesText, Event};
use quick_xml::XmlVersion;
use reqwest::{
    Url,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};

#[derive(Clone)]
pub struct DavClient {
    client: reqwest::Client,
    collection_url: Url,
}

#[derive(Debug, thiserror::Error)]
pub enum DavError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("utf8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("invalid URL: {0}")]
    InvalidUrl(#[from] url::ParseError),

    #[error("unexpected response: {0}")]
    UnexpectedResponse(String),

    #[error("error parsing resource references: {0}")]
    ParseReference(#[from] ParseResourceRefError),
    #[error("error parsing multiget response: {0}")]
    ParseMultiget(#[from] ParseMultigetError),
}

#[derive(Debug, Clone)]
pub struct ResourceRef {
    pub href: String,
    pub etag: Option<String>,
}

impl DavClient {
    pub fn new(collection_url: Url, username: &str, password: &str) -> Result<Self, DavError> {
        let mut headers = HeaderMap::new();

        let auth = format!("{}:{}", username, password);
        let encoded = general_purpose::STANDARD.encode(auth.as_bytes());
        let value = HeaderValue::from_str(&format!("Basic {}", encoded))
            .map_err(|_| DavError::UnexpectedResponse("invalid auth header".into()))?;

        headers.insert(AUTHORIZATION, value);

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            client,
            collection_url: collection_url.into(),
        })
    }

    pub async fn list_resources(
        &self,
        query_type: &str,
        dav_specialisation_namespace: &str,
    ) -> Result<Vec<ResourceRef>, DavError> {
        let mut writer = quick_xml::Writer::new(Cursor::new(Vec::<u8>::new()));

        writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))
            .unwrap();

        writer
            .create_element(query_type)
            .with_attribute(("xmlns:d", "DAV:"))
            .with_attribute(("xmlns:c", dav_specialisation_namespace))
            .write_inner_content(|writer| {
                writer
                    .create_element("d:prop")
                    .write_inner_content(|writer| {
                        writer.create_element("d:getetag").write_empty()?;
                        Ok(())
                    })?;
                Ok(())
            })
            .expect("Writing to a Vec is unlikely to fail");
        let xml = writer.into_inner().into_inner();

        let resp = self
            .client
            .request(
                reqwest::Method::from_bytes(b"REPORT").unwrap(),
                self.collection_url.clone(),
            )
            .header("Depth", "1")
            .header("Content-Type", "application/xml")
            .body(xml)
            .send()
            .await?
            .error_for_status()?;

        let bytes = resp.bytes().await?;
        Ok(parse_resource_refs(&bytes)?)
    }

    pub async fn list_resources_by_property(
        &self,
        query_type: &str,
        dav_specialisation_namespace: &str,
        component: &str,
        property: &str,
        text_match: &str,
    ) -> Result<Vec<ResourceRef>, DavError> {
        let mut writer = quick_xml::Writer::new(Cursor::new(Vec::<u8>::new()));

        writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))
            .unwrap();

        writer
            .create_element(query_type)
            .with_attribute(("xmlns:d", "DAV:"))
            .with_attribute(("xmlns:c", dav_specialisation_namespace))
            .write_inner_content(|writer| {
                writer
                    .create_element("d:prop")
                    .write_inner_content(|writer| {
                        writer.create_element("d:getetag").write_empty()?;
                        Ok(())
                    })?;
                writer
                    .create_element("c:filter")
                    .write_inner_content(|writer| {
                        writer
                            .create_element("c:comp-filter")
                            .with_attribute(("name", "VCALENDAR"))
                            .write_inner_content(|writer| {
                                writer
                                    .create_element("c:comp-filter")
                                    .with_attribute(("name", component))
                                    .write_inner_content(|writer| {
                                        writer
                                            .create_element("c:prop-filter")
                                            .with_attribute(("name", property))
                                            .write_inner_content(|writer| {
                                                writer
                                                    .create_element("c:text-match")
                                                    .with_attribute((
                                                        "collation",
                                                        "i;ascii-casemap",
                                                    ))
                                                    .write_text_content(BytesText::new(
                                                        text_match,
                                                    ))?;
                                                Ok(())
                                            })?;
                                        Ok(())
                                    })?;
                                Ok(())
                            })?;
                        Ok(())
                    })?;
                Ok(())
            })
            .expect("Writing to a Vec is unlikely to fail");

        let response = self
            .client
            .request(
                reqwest::Method::from_bytes(b"REPORT").unwrap(),
                self.collection_url.clone(),
            )
            .header("Depth", "1")
            .header("Content-Type", "application/xml")
            .body(writer.into_inner().into_inner())
            .send()
            .await?
            .error_for_status()?;

        Ok(parse_resource_refs(&response.bytes().await?)?)
    }

    pub async fn create_resource(
        &self,
        resource_name: &str,
        content: String,
        content_type: &str,
    ) -> Result<(), DavError> {
        let mut resource_url = self.collection_url.clone();
        let path = format!(
            "{}/{}",
            resource_url.path().trim_end_matches('/'),
            resource_name
        );
        resource_url.set_path(&path);

        self.client
            .put(resource_url)
            .header("Content-Type", content_type)
            .header("If-None-Match", "*")
            .body(content)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    pub async fn delete_resource(&self, href: &str) -> Result<(), DavError> {
        self.client
            .delete(self.collection_url.join(href)?)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    pub async fn multiget(
        &self,
        resources: &[ResourceRef],
        query_type: &str,
        dav_specialisation_namespace: &str,
        dav_specialisation_data_namespace: &str,
    ) -> Result<Vec<String>, DavError> {
        let mut writer = quick_xml::Writer::new(Cursor::new(Vec::<u8>::new()));

        writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))
            .unwrap();

        writer
            .create_element(query_type)
            .with_attribute(("xmlns:d", "DAV:"))
            .with_attribute(("xmlns:c", dav_specialisation_namespace))
            .write_inner_content(|writer| {
                writer
                    .create_element("d:prop")
                    .write_inner_content(|writer| {
                        writer.create_element("d:getetag").write_empty()?;
                        writer
                            .create_element(dav_specialisation_data_namespace)
                            .write_empty()?;
                        Ok(())
                    })?;
                for contact in resources {
                    writer
                        .create_element("d:href")
                        .write_text_content(BytesText::new(&contact.href))?;
                }
                Ok(())
            })
            .unwrap();

        let xml = writer.into_inner().into_inner();

        let resp = self
            .client
            .request(
                reqwest::Method::from_bytes(b"REPORT").unwrap(),
                self.collection_url.clone(),
            )
            .header("Depth", "1")
            .header("Content-Type", "application/xml")
            .body(xml)
            .send()
            .await?
            .error_for_status()?;

        let bytes = resp.bytes().await?;

        let resources = parse_multiget(&bytes)?;

        Ok(resources)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseMultigetError {
    #[error("xml error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("missing declaration")]
    MissingDecl,
    #[error("unexpected end of file")]
    UnexpectedEof,
    #[error("encoding error")]
    EncodingError(#[from] quick_xml::encoding::EncodingError),
    #[error("missing content")]
    MissingContent,
}

fn parse_multiget(xml: &[u8]) -> Result<Vec<String>, ParseMultigetError> {
    let mut reader = quick_xml::Reader::from_reader(xml);

    match reader.read_event()? {
        Event::Decl(_) => {}
        _ => return Err(ParseMultigetError::MissingDecl),
    }

    // Find start of multistatus
    loop {
        match reader.read_event()? {
            Event::Start(start) => {
                if start.local_name().as_ref() == b"multistatus" {
                    break;
                }
            }
            Event::Eof => return Ok(vec![]),
            _ => {}
        }
    }

    let mut results = Vec::new();

    loop {
        match reader.read_event()? {
            Event::Start(start) => {
                if start.local_name().as_ref() == b"response" {
                    results.push(parse_multiget_response(&mut reader)?);
                }
            }
            Event::End(end) => {
                if end.local_name().as_ref() == b"multistatus" {
                    break;
                }
            }
            Event::Eof => return Err(ParseMultigetError::UnexpectedEof),
            _ => {}
        }
    }

    Ok(results)
}

fn parse_multiget_response(
    reader: &mut quick_xml::Reader<&[u8]>,
) -> Result<String, ParseMultigetError> {
    let mut content = None;
    loop {
        match reader.read_event()? {
            Event::Start(start) => match start.local_name().as_ref() {
                b"address-data" => {
                    let end = start.to_end().into_owned();
                    let text = reader.read_text(end.name())?;
                    let decoded = text.decode()?;
                    content = Some(
                        quick_xml::escape::unescape(&decoded)
                            .map_err(quick_xml::Error::from)?
                            .into_owned(),
                    );
                }
                _ => {}
            },
            Event::End(end) => {
                if end.local_name().as_ref() == b"response" {
                    break;
                }
            }
            Event::Eof => {
                return Err(ParseMultigetError::UnexpectedEof);
            }
            _ => {}
        }
    }

    content.ok_or_else(|| ParseMultigetError::MissingContent)
}

#[derive(Debug, thiserror::Error)]
pub enum ParseResourceRefError {
    #[error("xml error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("missing declaration")]
    MissingDecl,
    #[error("unexpected end of file")]
    UnexpectedEof,
    #[error("unexpected event")]
    UnexpectedEvent,
    #[error("missing href")]
    MissingHref,
    #[error("encoding error")]
    EncodingError(#[from] quick_xml::encoding::EncodingError),
}

fn parse_resource_refs(xml: &[u8]) -> Result<Vec<ResourceRef>, ParseResourceRefError> {
    let mut reader = quick_xml::Reader::from_reader(xml);

    match reader.read_event()? {
        Event::Decl(_) => {}
        _ => return Err(ParseResourceRefError::MissingDecl),
    }

    // Find start of multistatus
    loop {
        match reader.read_event()? {
            Event::Start(start) => {
                if start.local_name().as_ref() == b"multistatus" {
                    break;
                }
            }
            Event::Eof => return Ok(vec![]),
            _ => {}
        }
    }

    let mut results = Vec::new();

    loop {
        match reader.read_event()? {
            Event::Start(start) => {
                if start.local_name().as_ref() == b"response" {
                    results.push(parse_single_ref(&mut reader)?);
                }
            }
            Event::End(end) => {
                if end.local_name().as_ref() == b"multistatus" {
                    break;
                }
            }
            Event::Eof => return Err(ParseResourceRefError::UnexpectedEof),
            _ => {}
        }
    }

    Ok(results)
}

fn parse_single_ref(
    reader: &mut quick_xml::Reader<&[u8]>,
) -> Result<ResourceRef, ParseResourceRefError> {
    let mut href = None;
    let mut etag = None;

    loop {
        match reader.read_event()? {
            Event::Start(start) => match start.local_name().as_ref() {
                b"href" => {
                    if let Event::Text(t) = reader.read_event()? {
                        href = Some(t.xml_content(XmlVersion::Explicit1_0)?);
                    }
                    match reader.read_event()? {
                        Event::End(end) => {
                            if end.local_name().as_ref() != b"href" {
                                return Err(ParseResourceRefError::UnexpectedEvent);
                            }
                        }
                        _ => {
                            return Err(ParseResourceRefError::UnexpectedEvent);
                        }
                    }
                }
                b"getetag" => {
                    if let Event::Text(t) = reader.read_event()? {
                        etag = Some(t.xml_content(XmlVersion::Explicit1_0)?);
                    }
                    match reader.read_event()? {
                        Event::End(end) => {
                            if end.local_name().as_ref() != b"getetag" {
                                return Err(ParseResourceRefError::UnexpectedEvent);
                            }
                        }
                        _ => return Err(ParseResourceRefError::UnexpectedEvent),
                    }
                }
                _ => {}
            },
            Event::End(end) => {
                if end.local_name().as_ref() == b"response" {
                    break;
                }
            }
            Event::Eof => {
                return Err(ParseResourceRefError::UnexpectedEof);
            }
            _ => {}
        }
    }

    if let Some(href) = href {
        Ok(ResourceRef {
            href: href.to_string(),
            etag: etag.map(|etag| etag.to_string()),
        })
    } else {
        Err(ParseResourceRefError::MissingHref)
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_multiget, parse_resource_refs};

    #[test]
    fn parses_resource_refs_with_arbitrary_namespace_prefixes() {
        let xml = br#"<?xml version="1.0" encoding="utf-8"?>
<dav:multistatus xmlns:dav="DAV:">
  <dav:response>
    <dav:href>/addressbooks/user/default/contact.vcf</dav:href>
    <dav:propstat>
      <dav:prop>
        <dav:getetag>"123"</dav:getetag>
      </dav:prop>
    </dav:propstat>
  </dav:response>
</dav:multistatus>"#;

        let resources = parse_resource_refs(xml).expect("response should parse");

        assert_eq!(resources.len(), 1);
        assert_eq!(
            resources[0].href,
            "/addressbooks/user/default/contact.vcf"
        );
        assert_eq!(resources[0].etag.as_deref(), Some("\"123\""));
    }

    #[test]
    fn parses_multiget_with_lowercase_namespace_prefixes() {
        let xml = br#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
  <d:response>
    <d:href>/addressbooks/user/default/contact.vcf</d:href>
    <d:propstat>
      <d:prop>
        <c:address-data>BEGIN:VCARD
VERSION:3.0
FN:Example &amp; Contact
END:VCARD</c:address-data>
      </d:prop>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

        let contacts = parse_multiget(xml).expect("response should parse");

        assert_eq!(contacts.len(), 1);
        assert!(contacts[0].contains("FN:Example & Contact"));
        assert!(contacts[0].ends_with("END:VCARD"));
    }
}
