//! Adapter for Starface phone systems.

use http::{HeaderMap, StatusCode, Uri, header::InvalidHeaderValue, uri::InvalidUri};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::collections::HashMap;
use std::fmt::Write;

use crate::ContactDestination;

mod batch;
mod contact;

/// A failed adapter operation with its underlying cause.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Missing authority in URI")]
    MissingAuthority,
    #[error("Missing scheme in URI")]
    MissingScheme,
    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("Wrong username or password")]
    AuthenticationFailed,
    #[error("Starface returned an error for unknown reason: {0}\n{1}")]
    UnknownError(StatusCode, String),
    #[error("Invalid URI: {0:?}")]
    InvalidUri(#[from] InvalidUri),
    #[error("Invalid header value: {0}")]
    InvalidHeaderValue(#[from] InvalidHeaderValue),
    #[error("Starface has no contact tags")]
    MissingContactTag,
    #[error("Refusing to sync an empty contact list to Starface")]
    EmptyContactList,
}

/// Raw connection settings serialized at the application boundary.
#[derive(Serialize, Deserialize)]
pub struct AccessData {
    pub url: String,
    pub username: String,
    pub password: String,
}

impl Destination {
    /// Connects using the supplied settings and checks remote access.
    pub async fn new(access_data: AccessData) -> Result<Self, Error> {
        let uri: Uri = access_data.url.parse()?;
        let username = access_data.username;
        let password = access_data.password;

        let mut headers = HeaderMap::new();

        headers.append(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/json"),
        );
        headers.append("X-Version", http::HeaderValue::from_static("2"));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        let login_endpoint = format!(
            "{}://{}{}",
            uri.scheme_str().ok_or(Error::MissingScheme)?,
            uri.authority().ok_or(Error::MissingAuthority)?,
            "/rest/login"
        );

        let mut nonce = client
            .get(&login_endpoint)
            .send()
            .await?
            .json::<LoginNonce>()
            .await?;

        let password_hash = to_hex(Sha512::digest(password).iter());

        let hash_input = format!("{}{}{}", username, nonce.nonce, password_hash);

        let hsecret = Sha512::digest(hash_input);

        let secret = format!("{}:{}", username, to_hex(hsecret.iter()));

        nonce.secret = Some(secret);

        let response = client.post(&login_endpoint).json(&nonce).send().await?;

        match response.status() {
            StatusCode::OK => {}
            StatusCode::BAD_REQUEST => return Err(Error::AuthenticationFailed),
            status => {
                return Err(Error::UnknownError(
                    status,
                    response.text().await.unwrap_or_else(|_| String::new()),
                ));
            }
        }

        let login_response = response.json::<LoginResponse>().await?;

        let mut headers = HeaderMap::new();

        headers.append(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/json"),
        );
        headers.append("authToken", login_response.token.parse()?);

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        let tag_endpoint = format!(
            "{}://{}/rest/contacts/tags",
            uri.scheme_str().ok_or(Error::MissingScheme)?,
            uri.authority().ok_or(Error::MissingAuthority)?,
        );

        let tags = client
            .get(&tag_endpoint)
            .send()
            .await?
            .json::<Vec<Tag>>()
            .await?;

        Ok(Self {
            client,
            uri,
            tag: tags.last().ok_or(Error::MissingContactTag)?.clone(),
        })
    }

    async fn upload_contact(&self, contact: &contact::Contact) -> Result<(), Error> {
        let contacts_endpoint = format!(
            "{}://{}/rest/contacts",
            self.uri.scheme_str().ok_or(Error::MissingScheme)?,
            self.uri.authority().ok_or(Error::MissingAuthority)?,
        );

        let upload = contact.to_upload(&self.tag);

        let response = self
            .client
            .post(&contacts_endpoint)
            .json(&upload)
            .send()
            .await?;

        response.error_for_status()?;

        Ok(())
    }

    async fn delete_contact(&self, id: &str) -> Result<(), Error> {
        let contacts_endpoint = format!(
            "{}://{}/rest/contacts/{}",
            self.uri.scheme_str().ok_or(Error::MissingScheme)?,
            self.uri.authority().ok_or(Error::MissingAuthority)?,
            id
        );

        let response = self.client.delete(&contacts_endpoint).send().await?;

        response.error_for_status()?;

        Ok(())
    }
}

impl ContactResponse {
    fn parse(self) -> Vec<contact::Contact> {
        self.contacts
            .into_iter()
            // We don't want to try and delete contacts which are just system users
            .filter(|raw| raw.additional_values.user_account_id.is_none())
            .map(|raw| {
                contact::Contact::from_raw(
                    raw,
                    &self.summary_block_schema,
                    &self.phone_numbers_block_schema,
                )
            })
            .collect()
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct AdditionalValues {
    #[serde(rename = "USER_ACCOUNT_ID", skip_serializing_if = "Option::is_none")]
    user_account_id: Option<String>,
}

impl ContactDestination for Destination {
    type Error = Error;

    async fn export_contacts(
        &self,
        contacts: impl Iterator<Item = &crate::contact::Contact>,
    ) -> Result<(), Self::Error> {
        let contacts = batch::Batch::try_from_contacts(contacts)?;

        let contacts_endpoint = format!(
            "{}://{}/rest/contacts?pagesize=40",
            self.uri.scheme_str().ok_or(Error::MissingScheme)?,
            self.uri.authority().ok_or(Error::MissingAuthority)?,
        );

        // println!(
        //     "{}",
        //     self.client
        //         .get(&contacts_endpoint)
        //         .send()
        //         .await?
        //         .text()
        //         .await?
        // );
        let response = self
            .client
            .get(&contacts_endpoint)
            .send()
            .await?
            .json::<ContactResponse>()
            .await?;

        println!("Loading existing contacts");
        let total_pages = response.metadata.total_pages;
        let mut existing_contacts = HashMap::with_capacity(40 * total_pages);
        for contact in response.parse() {
            existing_contacts.insert(contact, false);
        }

        // DO NOT PARALLELIZE THIS LOOP
        // Last time I tried, the starface would just delete all contacts
        for page in 1..total_pages {
            let page_endpoint = format!("{}&page={}", contacts_endpoint, page);
            let page_response = self
                .client
                .get(&page_endpoint)
                .send()
                .await?
                .json::<ContactResponse>()
                .await?;

            for contact in page_response.parse() {
                existing_contacts.insert(contact, false);
            }
        }

        println!(
            "Found {} existing contacts, updating...",
            existing_contacts.len()
        );

        let mut created = 0;
        let mut deleted = 0;
        let mut unchanged = 0;

        for contact in contacts {
            if let Some(found) = existing_contacts.get_mut(&contact) {
                *found = true;
                unchanged += 1;
            } else {
                self.upload_contact(&contact).await?;
                println!("Uploading: {:?}", contact);
                created += 1;
            }
        }

        for (contact, found) in existing_contacts {
            if !found {
                self.delete_contact(&contact.id).await?;
                println!("Deleting: {:?}", contact);
                deleted += 1;
            }
        }

        println!("Created: {}", created);
        println!("Deleted: {}", deleted);
        println!("Unchanged: {}", unchanged);

        Ok(())
    }
}

#[derive(Deserialize, Debug)]
struct ContactResponse {
    metadata: Metadata,
    #[serde(rename = "phoneNumbersBlockSchema")]
    phone_numbers_block_schema: PhoneSchema,
    #[serde(rename = "summaryBlockSchema")]
    summary_block_schema: SummarySchema,
    contacts: Vec<RawContact>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Metadata {
    page: usize,
    #[serde(rename = "totalPages")]
    total_pages: usize,
}

#[derive(Serialize, Deserialize, Debug)]
struct RawContact {
    id: String,
    #[serde(rename = "summaryValues")]
    summary_values: Vec<String>,
    #[serde(rename = "phoneNumberValues")]
    phone_numbers: Vec<String>,
    #[serde(rename = "additionalValues")]
    additional_values: AdditionalValues,
}

#[derive(Deserialize, Debug)]
struct PhoneSchema {
    // name: String,
    attributes: Vec<PhoneSchemaBlock>,
}

#[derive(Deserialize, Debug)]
struct SummarySchema {
    // name: String,
    attributes: Vec<SummarySchemaBlock>,
}

#[derive(Deserialize, Debug)]
struct PhoneSchemaBlock {
    name: PhoneSchemaType,
}

#[derive(Deserialize, Debug)]
struct SummarySchemaBlock {
    name: SummarySchemaType,
}

#[derive(Deserialize, Debug)]
enum PhoneSchemaType {
    #[serde(rename = "phone")]
    Phone,
    #[serde(rename = "mobile")]
    Mobile,
    #[serde(rename = "homephone")]
    HomePhone,
}

#[derive(Deserialize, Debug)]
enum SummarySchemaType {
    #[serde(rename = "familyname")]
    FamilyName,
    #[serde(rename = "firstname")]
    FirstName,
    #[serde(rename = "company")]
    Company,
}

#[derive(Debug, Serialize, Deserialize)]
struct LoginNonce {
    #[serde(rename = "loginType")]
    login_type: String,
    nonce: String,
    secret: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    token: String,
}

/// An authenticated destination ready to export contacts.
pub struct Destination {
    uri: Uri,
    client: reqwest::Client,
    tag: Tag,
}

fn to_hex<'a>(value: impl ExactSizeIterator<Item = &'a u8>) -> String {
    let mut hex = String::with_capacity(2 * value.len());
    for byte in value {
        write!(&mut hex, "{:02x}", byte).expect("formatting into a String cannot fail");
    }
    hex
}

#[derive(Debug, Serialize)]
struct UploadContact<'a> {
    blocks: Vec<Block<'a>>,
    editable: bool,
    tags: Vec<&'a Tag>,
    id: &'a str,
}

#[derive(Debug, Serialize)]
struct Block<'a> {
    name: &'a str,
    attributes: Vec<Attribute<'a>>,
}

#[derive(Debug, Serialize)]
struct Attribute<'a> {
    name: &'a str,
    value: &'a str,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Tag {
    id: String,
    name: String,
    alias: String,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::mpsc,
    };

    use super::{Destination, Tag};
    use crate::{ContactDestination, contact};

    #[tokio::test]
    async fn rejected_replacement_does_not_delete_the_existing_contact() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let uri = format!("http://{}", listener.local_addr().unwrap())
            .parse()
            .unwrap();
        let (send, mut requests) = mpsc::unbounded_channel();
        let server = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buffer = Vec::new();
                let mut chunk = [0; 4096];
                let header_end = loop {
                    let count = stream.read(&mut chunk).await.unwrap();
                    assert_ne!(count, 0);
                    buffer.extend_from_slice(&chunk[..count]);
                    if let Some(offset) = buffer.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                        break offset + 4;
                    }
                };
                let headers = String::from_utf8(buffer[..header_end].to_vec()).unwrap();
                let content_length: usize = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse().unwrap())
                    })
                    .unwrap_or(0);
                while buffer.len() < header_end + content_length {
                    let count = stream.read(&mut chunk).await.unwrap();
                    assert_ne!(count, 0);
                    buffer.extend_from_slice(&chunk[..count]);
                }
                let method = headers.split_whitespace().next().unwrap();
                send.send(method.to_owned()).unwrap();
                let (status, body) = if method == "GET" {
                    (
                        "200 OK",
                        r#"{
                    "metadata": {"page": 0, "totalPages": 1},
                    "phoneNumbersBlockSchema": {"attributes": []},
                    "summaryBlockSchema": {"attributes": [{"name": "firstname"}]},
                    "contacts": [{"id": "existing", "summaryValues": ["Old name"],
                        "phoneNumberValues": [], "additionalValues": {}}]
                }"#,
                    )
                } else {
                    ("500 Internal Server Error", "replacement rejected")
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let destination = Destination {
            uri,
            client: reqwest::Client::builder().no_proxy().build().unwrap(),
            tag: Tag {
                id: "test".into(),
                name: "test".into(),
                alias: "test".into(),
            },
        };
        let contacts = [contact::Contact {
            name: contact::Name {
                given_name: Some("New name".into()),
                ..contact::Name::default()
            },
            ..contact::Contact::default()
        }];
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            destination.export_contacts(contacts.iter()),
        )
        .await
        .unwrap();
        assert!(result.is_err());
        assert_eq!(requests.try_recv().unwrap(), "GET");
        assert_eq!(requests.try_recv().unwrap(), "POST");
        assert!(
            requests.try_recv().is_err(),
            "a failed upload must stop before DELETE"
        );
        // Deletion failures also propagate instead of producing a successful sync.
        assert!(destination.delete_contact("existing").await.is_err());
        assert_eq!(requests.try_recv().unwrap(), "DELETE");
        server.abort();
    }
}
