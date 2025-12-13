use std::hash::Hash;

use base64::{Engine as _, engine::general_purpose};
use quick_xml::events::Event;
use reqwest::{
    Url,
    header::{AUTHORIZATION, HeaderMap, HeaderValue, IF_MATCH},
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
        let body = r#"
<?xml version="1.0" encoding="utf-8"?>
<c:addressbook-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:carddav">
  <d:prop>
    <d:getetag />
  </d:prop>
</c:addressbook-query>
"#;

        let resp = self
            .client
            .request(
                reqwest::Method::from_bytes(b"REPORT").unwrap(),
                self.addressbook_url.clone(),
            )
            .header("Depth", "1")
            .header("Content-Type", "application/xml")
            .body(body)
            .send()
            .await?
            .error_for_status()?;

        let bytes = resp.bytes().await?;
        Ok(parse_contact_refs(&bytes)?)
    }

    /// ---------- Read contact ----------
    pub async fn get_contact(&self, href: &str) -> Result<String, CardDavError> {
        let url = join_url(&self.addressbook_url, href);

        let resp = self.client.get(url).send().await?.error_for_status()?;
        let bytes = resp.bytes().await?;

        Ok(String::from_utf8(bytes.to_vec())?)
    }

    /// ---------- Upload / update contact ----------
    pub async fn put_contact(
        &self,
        href: &str,
        vcard: &str,
        etag: Option<&str>,
    ) -> Result<(), CardDavError> {
        let url = join_url(&self.addressbook_url, href);

        let mut req = self
            .client
            .put(url)
            .header("Content-Type", "text/vcard; charset=utf-8")
            .body(vcard.to_owned());

        if let Some(etag) = etag {
            req = req.header(IF_MATCH, etag);
        }

        req.send().await?.error_for_status()?;
        Ok(())
    }

    /// ---------- Delete contact ----------
    pub async fn delete_contact(&self, href: &str) -> Result<(), CardDavError> {
        let url = join_url(&self.addressbook_url, href);

        self.client.delete(url).send().await?.error_for_status()?;

        Ok(())
    }
}

/// ---------- Helpers ----------

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
    let str = str::from_utf8(xml).unwrap();
    println!("{}", str);
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
    //     let mut href: Option<String> = None;
    //     let mut etag: Option<String> = None;
    //     match reader.read_event()? {
    //         Event::Start(e) => match e.name().as_ref() {
    //             b"href" => {
    //                 if let Event::Text(t) = reader.read_event_into(&mut buf)? {
    //                     href = Some(t.unescape()?.to_string());
    //                 }
    //             }
    //             b"getetag" => {
    //                 if let Event::Text(t) = reader.read_event_into(&mut buf)? {
    //                     etag = Some(t.unescape()?.to_string());
    //                 }
    //             }
    //             _ => {}
    //         },
    //         Event::End(e) if e.name().as_ref() == b"response" => {
    //             if let Some(href) = href.take() {
    //                 results.push(ContactRef {
    //                     href,
    //                     etag: etag.take(),
    //                 });
    //             }
    //         }
    //         Event::Eof => break,
    //         _ => {}
    //     }
    //     buf.clear();
}

fn parse_single_ref(
    reader: &mut quick_xml::Reader<&[u8]>,
) -> Result<ContactRef, ParseContactRefError> {
    let mut href = None;
    let mut etag = None;
    println!("Parsing single ref");

    loop {
        match reader.read_event()? {
            Event::Start(start) => match start.name().as_ref() {
                b"D:href" => {
                    println!("Found HREF");
                    if let Event::Text(t) = reader.read_event()? {
                        href = Some(t.xml_content()?);
                    }
                    match reader.read_event()? {
                        Event::End(end) => {
                            if end.name().as_ref() != b"D:href" {
                                return Err(ParseContactRefError::UnexpectedEvent);
                            }
                        }
                        event => {
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
                start => {
                    let start = str::from_utf8(start).unwrap();
                    println!("start: {:?}", start);
                }
            },
            Event::End(end) => {
                if end.name().as_ref() == b"D:response" {
                    break;
                }
            }
            Event::Eof => {
                return Err(ParseContactRefError::UnexpectedEof);
            }
            event => {
                println!("event: {:?}", event);
            }
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

fn join_url(base: &str, href: &str) -> String {
    if href.starts_with("http") {
        href.to_string()
    } else {
        format!(
            "{}/{}",
            base.trim_end_matches('/'),
            href.trim_start_matches('/')
        )
    }
}
