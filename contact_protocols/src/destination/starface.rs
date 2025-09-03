use http::{HeaderMap, StatusCode, Uri, uri::InvalidUri};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::collections::HashMap;
use std::fmt::Write;
use thiserror::Error;

use crate::{ContactDestination, destination::starface::contact::StarfaceContact};

mod contact;

#[derive(Debug, Error)]
pub enum StarfaceError {
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
}

#[derive(Serialize, Deserialize)]
pub struct StarfaceAccessData {
    pub url: String,
    pub username: String,
    pub password: String,
}

impl StarfaceDestination {
    pub async fn new(access_data: StarfaceAccessData) -> Result<Self, StarfaceError> {
        let uri: Uri = access_data.url.parse()?;
        let username = access_data.username;
        let password = access_data.password;

        let mut headers = HeaderMap::new();

        headers.append(
            http::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.append("X-Version", "2".parse().unwrap());

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        let login_endpoint = format!(
            "{}://{}{}",
            uri.scheme_str().ok_or(StarfaceError::MissingScheme)?,
            uri.authority().ok_or(StarfaceError::MissingAuthority)?,
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
            StatusCode::BAD_REQUEST => return Err(StarfaceError::AuthenticationFailed),
            status => {
                return Err(StarfaceError::UnknownError(
                    status,
                    response.text().await.unwrap_or_else(|_| String::new()),
                ));
            }
        }

        let login_response = response.json::<LoginResponse>().await?;

        let mut headers = HeaderMap::new();

        headers.append(
            http::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.append("authToken", login_response.token.parse().unwrap());

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        let tag_endpoint = format!(
            "{}://{}/rest/contacts/tags",
            uri.scheme_str().ok_or(StarfaceError::MissingScheme)?,
            uri.authority().ok_or(StarfaceError::MissingAuthority)?,
        );

        let tags = client
            .get(&tag_endpoint)
            .send()
            .await?
            .json::<Vec<Tag>>()
            .await?;

        // println!("Tags: {:?}", tags);

        Ok(Self {
            client,
            uri,
            tag: tags.last().unwrap().clone(),
        })
    }

    async fn upload_contact(&self, contact: &StarfaceContact) -> Result<(), StarfaceError> {
        let contacts_endpoint = format!(
            "{}://{}/rest/contacts",
            self.uri.scheme_str().ok_or(StarfaceError::MissingScheme)?,
            self.uri
                .authority()
                .ok_or(StarfaceError::MissingAuthority)?,
        );

        let upload = contact.to_upload(&self.tag);

        let _response = self
            .client
            .post(&contacts_endpoint)
            .json(&upload)
            .send()
            .await?;

        Ok(())
    }

    async fn delete_contact(&self, id: &str) -> Result<(), StarfaceError> {
        let contacts_endpoint = format!(
            "{}://{}/rest/contacts/{}",
            self.uri.scheme_str().ok_or(StarfaceError::MissingScheme)?,
            self.uri
                .authority()
                .ok_or(StarfaceError::MissingAuthority)?,
            id
        );

        let _response = self.client.delete(&contacts_endpoint).send().await?;

        Ok(())
    }
}

impl ContactResponse {
    fn parse(self) -> Vec<StarfaceContact> {
        self.contacts
            .into_iter()
            // We don't want to try and delete contacts which are just system users
            .filter(|raw| raw.additional_values.user_account_id.is_none())
            .map(|raw| {
                StarfaceContact::from_raw(
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

impl ContactDestination for StarfaceDestination {
    type Error = StarfaceError;

    async fn export_contacts(
        &self,
        contacts: impl Iterator<Item = &crate::contact::Contact>,
    ) -> Result<(), Self::Error> {
        let contacts_endpoint = format!(
            "{}://{}/rest/contacts?pagesize=40",
            self.uri.scheme_str().ok_or(StarfaceError::MissingScheme)?,
            self.uri
                .authority()
                .ok_or(StarfaceError::MissingAuthority)?,
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
            let page_endpoint = format!("{}&page={}", &contacts_endpoint, page);
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

        for contact in contacts.filter_map(|contact| StarfaceContact::from_contact(contact)) {
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
    contacts: Vec<RawStarfaceContact>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Metadata {
    page: usize,
    #[serde(rename = "totalPages")]
    total_pages: usize,
}

#[derive(Serialize, Deserialize, Debug)]
struct RawStarfaceContact {
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

pub struct StarfaceDestination {
    uri: Uri,
    client: reqwest::Client,
    tag: Tag,
}

fn to_hex<'a>(value: impl ExactSizeIterator<Item = &'a u8>) -> String {
    let mut hex = String::with_capacity(2 * value.len());
    for byte in value {
        write!(&mut hex, "{:02x}", byte).unwrap();
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
