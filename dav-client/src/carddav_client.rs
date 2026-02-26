use std::{hash::Hash, io::Cursor};

use base64::{Engine as _, engine::general_purpose};
use quick_xml::events::{BytesDecl, BytesStart, BytesText, Event};
use reqwest::{
    Url,
    header::{AUTHORIZATION, HeaderMap, HeaderValue, IF_MATCH},
};

use crate::{
    contact::Contact,
    vcard::parse_vcard,
    vobject::vcard::{VCard, VCardError},
};

/// ---------- Error handling ----------

#[derive(Debug, thiserror::Error)]
pub enum CardDavError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("utf8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("unexpected response: {0}")]
    UnexpectedResponse(String),

    #[error("error parsing contact refs: {0}")]
    ParseContactRef(#[from] ParseContactRefError),

    #[error("error parsing multiget response: {0}")]
    ParseMultiget(#[from] ParseMultigetError),
}

/// ---------- Data types ----------

#[derive(Debug, Clone)]
pub struct ContactRef {
    pub href: String,
    pub etag: Option<String>,
}

#[derive(Clone)]
pub struct CardDavClient {
    client: reqwest::Client,
    addressbook_url: Url,
}

/// ---------- Client ----------

impl CardDavClient {
    pub fn new(addressbook_url: Url, username: &str, password: &str) -> Result<Self, CardDavError> {
        let mut headers = HeaderMap::new();

        let auth = format!("{}:{}", username, password);
        let encoded = general_purpose::STANDARD.encode(auth.as_bytes());
        let value = HeaderValue::from_str(&format!("Basic {}", encoded))
            .map_err(|_| CardDavError::UnexpectedResponse("invalid auth header".into()))?;

        headers.insert(AUTHORIZATION, value);

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self {
            client,
            addressbook_url: addressbook_url.into(),
        })
    }

    /// ---------- List contacts ----------
    pub async fn list_contacts(&self) -> Result<Vec<ContactRef>, CardDavError> {
        let mut writer = quick_xml::Writer::new(Cursor::new(Vec::<u8>::new()));

        writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))
            .unwrap();

        writer
            .create_element("c:addressbook-query")
            .with_attribute(("xmlns:d", "DAV:"))
            .with_attribute(("xmlns:c", "urn:ietf:params:xml:ns:carddav"))
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
                self.addressbook_url.clone(),
            )
            .header("Depth", "1")
            .header("Content-Type", "application/xml")
            .body(xml)
            .send()
            .await?
            .error_for_status()?;

        let bytes = resp.bytes().await?;
        Ok(parse_contact_refs(&bytes)?)
    }

    pub async fn fetch_contacts(
        &self,
        contacts: &[ContactRef],
    ) -> Result<Vec<VCard>, CardDavError> {
        let mut writer = quick_xml::Writer::new(Cursor::new(Vec::<u8>::new()));

        writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))
            .unwrap();

        writer
            .create_element("c:addressbook-multiget")
            .with_attribute(("xmlns:d", "DAV:"))
            .with_attribute(("xmlns:c", "urn:ietf:params:xml:ns:carddav"))
            .write_inner_content(|writer| {
                writer
                    .create_element("d:prop")
                    .write_inner_content(|writer| {
                        writer.create_element("d:getetag").write_empty()?;
                        writer.create_element("c:address-data").write_empty()?;
                        Ok(())
                    })?;
                for contact in contacts {
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
                self.addressbook_url.clone(),
            )
            .header("Depth", "1")
            .header("Content-Type", "application/xml")
            .body(xml)
            .send()
            .await?
            .error_for_status()?;

        let bytes = resp.bytes().await?;

        let contacts = parse_multiget(&bytes)?;

        Ok(contacts)
    }
}

// ---------- Helpers ----------

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
    #[error("parse vcard error: {0}")]
    ParseVCard(#[from] VCardError),
}

fn parse_multiget(xml: &[u8]) -> Result<Vec<VCard>, ParseMultigetError> {
    let mut reader = quick_xml::Reader::from_reader(xml);

    match reader.read_event()? {
        Event::Decl(_) => {}
        _ => return Err(ParseMultigetError::MissingDecl),
    }

    // Find start of multistatus
    loop {
        match reader.read_event()? {
            Event::Start(start) => {
                if start.name().as_ref() == b"D:multistatus" {
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
                if start.name().as_ref() == b"D:response" {
                    results.push(parse_multiget_response(&mut reader)?);
                }
            }
            Event::End(end) => {
                if end.name().as_ref() == b"D:multistatus" {
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
) -> Result<VCard, ParseMultigetError> {
    let mut content = None;
    loop {
        match reader.read_event()? {
            Event::Start(start) => match start.name().as_ref() {
                b"C:address-data" => {
                    if let Event::Text(t) = reader.read_event()? {
                        content = Some(t.decode()?.to_string());
                    }
                }
                _ => {}
            },
            Event::End(end) => {
                if end.name().as_ref() == b"D:response" {
                    break;
                }
            }
            Event::Eof => {
                return Err(ParseMultigetError::UnexpectedEof);
            }
            _ => {}
        }
    }

    if let Some(content) = content {
        let contact = VCard::parse(&content)?;
        Ok(contact)
    } else {
        Err(ParseMultigetError::MissingContent)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseContactRefError {
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

fn parse_contact_refs(xml: &[u8]) -> Result<Vec<ContactRef>, ParseContactRefError> {
    let mut reader = quick_xml::Reader::from_reader(xml);

    match reader.read_event()? {
        Event::Decl(_) => {}
        _ => return Err(ParseContactRefError::MissingDecl),
    }

    // Find start of multistatus
    loop {
        match reader.read_event()? {
            Event::Start(start) => {
                if start.name().as_ref() == b"D:multistatus" {
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
                if start.name().as_ref() == b"D:response" {
                    results.push(parse_single_ref(&mut reader)?);
                }
            }
            Event::End(end) => {
                if end.name().as_ref() == b"D:multistatus" {
                    break;
                }
            }
            Event::Eof => return Err(ParseContactRefError::UnexpectedEof),
            _ => {}
        }
    }

    Ok(results)
}

fn parse_single_ref(
    reader: &mut quick_xml::Reader<&[u8]>,
) -> Result<ContactRef, ParseContactRefError> {
    let mut href = None;
    let mut etag = None;

    loop {
        match reader.read_event()? {
            Event::Start(start) => match start.name().as_ref() {
                b"D:href" => {
                    if let Event::Text(t) = reader.read_event()? {
                        href = Some(t.xml_content()?);
                    }
                    match reader.read_event()? {
                        Event::End(end) => {
                            if end.name().as_ref() != b"D:href" {
                                return Err(ParseContactRefError::UnexpectedEvent);
                            }
                        }
                        _ => {
                            return Err(ParseContactRefError::UnexpectedEvent);
                        }
                    }
                }
                b"D:getetag" => {
                    if let Event::Text(t) = reader.read_event()? {
                        etag = Some(t.xml_content()?);
                    }
                    match reader.read_event()? {
                        Event::End(end) => {
                            if end.name().as_ref() != b"D:getetag" {
                                return Err(ParseContactRefError::UnexpectedEvent);
                            }
                        }
                        _ => return Err(ParseContactRefError::UnexpectedEvent),
                    }
                }
                _ => {}
            },
            Event::End(end) => {
                if end.name().as_ref() == b"D:response" {
                    break;
                }
            }
            Event::Eof => {
                return Err(ParseContactRefError::UnexpectedEof);
            }
            _ => {}
        }
    }

    if let Some(href) = href {
        Ok(ContactRef {
            href: href.to_string(),
            etag: etag.map(|etag| etag.to_string()),
        })
    } else {
        Err(ParseContactRefError::MissingHref)
    }
}
