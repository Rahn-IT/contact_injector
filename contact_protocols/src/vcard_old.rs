use std::{borrow::Cow, char};

use crate::{
    contact::{
        Address, AddressType, Attachment, Contact, ContactPhoto, Email, EmailType, Name, Phone,
        PhoneType, Url, UrlType,
    },
    vcard::util::{parse_date, parse_datetime, parse_string},
};
use base64::Engine;
use chrono::{NaiveDate, NaiveDateTime};
use nom::{
    IResult, Input, Mode, Parser,
    branch::alt,
    bytes::complete::{is_not, tag, take_while, take_while1},
    character::complete::{char, crlf, one_of},
    combinator::{map_res, opt},
    multi::{count, fold_many1, separated_list0, separated_list1},
    sequence::{delimited, preceded, terminated},
};
mod util;

pub fn remaining<I: Input>() -> impl Parser<I, Output = I, Error = nom::error::Error<I>> {
    Remaining {}
}

struct Remaining {}

impl<I> Parser<I> for Remaining
where
    I: nom::Input,
{
    type Output = I;

    type Error = nom::error::Error<I>;

    fn process<OM: nom::OutputMode>(
        &mut self,
        input: I,
    ) -> nom::PResult<OM, I, Self::Output, Self::Error> {
        Ok((input.take(0), OM::Output::bind(|| input)))
    }
}

pub fn parse_vcard(input: &str) -> IResult<&str, Contact> {
    delimited(
        (tag("BEGIN:VCARD"), crlf),
        fold_many1(
            parse_attribute,
            || Contact::default(),
            |mut contact, attr| {
                match attr {
                    ContactAttribute::None => {}
                    ContactAttribute::DisplayName(display_name) => {
                        contact.display_name = display_name;
                    }
                    ContactAttribute::Name(name) => {
                        contact.name = name;
                    }
                    ContactAttribute::Nickname(nickname) => {
                        contact.nickname = Some(nickname);
                    }
                    ContactAttribute::Phone(phone) => {
                        contact.phones.push(phone);
                    }
                    ContactAttribute::Address(address) => {
                        contact.addresses.push(address);
                    }
                    ContactAttribute::Birthday(birthday) => {
                        contact.birthday = Some(birthday);
                    }
                    ContactAttribute::Anniversary(anniversary) => {
                        contact.anniversary = Some(anniversary);
                    }
                    ContactAttribute::Photo(photo) => {
                        contact.photo = Some(photo);
                    }
                    ContactAttribute::Revision(revision) => {
                        contact.revision = Some(revision);
                    }
                    ContactAttribute::Uid(uid) => {
                        contact.uid = Some(uid);
                    }
                    ContactAttribute::Email(email) => {
                        contact.emails.push(email);
                    }
                    ContactAttribute::Org(org) => {
                        contact.org = Some(org);
                    }
                    ContactAttribute::Title(title) => {
                        contact.title = Some(title);
                    }
                    ContactAttribute::Url(url) => {
                        contact.urls.push(url);
                    }
                    ContactAttribute::Note(note) => {
                        contact.note = Some(note);
                    }
                    ContactAttribute::Attachment(attachment) => {
                        contact.attachments.push(attachment);
                    }
                }
                contact
            },
        ),
        opt(tag("END:VCARD")),
    )
    .parse(input)
}

enum ContactAttribute {
    None,
    DisplayName(String),
    Nickname(String),
    Name(Name),
    Phone(Phone),
    Address(Address),
    Birthday(NaiveDate),
    Anniversary(NaiveDate),
    Photo(ContactPhoto),
    Revision(NaiveDateTime),
    Uid(String),
    Email(Email),
    Org(String),
    Title(String),
    Url(Url),
    Note(String),
    Attachment(Attachment),
}

enum LineSearch {
    Searching,
    CarriageReturn,
    LineFeed,
}

fn parse_attribute(input: &str) -> IResult<&str, ContactAttribute> {
    let mut search_status = LineSearch::Searching;
    let mut attribute_raw = Cow::Borrowed("");
    let mut line_start = 0;
    for (index, ch) in input.char_indices() {
        match &search_status {
            LineSearch::Searching => {
                if ch == '\r' {
                    search_status = LineSearch::CarriageReturn;
                }
            }
            LineSearch::CarriageReturn => {
                if ch == '\n' {
                    search_status = LineSearch::LineFeed;
                }
            }
            LineSearch::LineFeed => {
                if ch == ' ' {
                    match &mut attribute_raw {
                        Cow::Borrowed(borrowed) => {
                            let mut owned = borrowed.to_owned();
                            owned.push_str(&input[line_start..index - 2]);
                            attribute_raw = Cow::Owned(owned);
                        }
                        Cow::Owned(owned) => {
                            owned.push_str(&input[line_start..index - 2]);
                        }
                    }
                    line_start = index + 1;
                    search_status = LineSearch::Searching;
                } else {
                    match &mut attribute_raw {
                        Cow::Borrowed(borrowed) => {
                            let mut owned = borrowed.to_owned();
                            owned.push_str(&input[line_start..index - 2]);
                            attribute_raw = Cow::Owned(owned);
                        }
                        Cow::Owned(owned) => {
                            owned.push_str(&input[line_start..index - 2]);
                        }
                    }
                    line_start = index;
                    break;
                }
            }
        }
    }

    let rest = &input[line_start..];

    match preceded(
        opt(terminated(
            alt((
                (tag("ITEM"), take_while1(char::is_numeric)).map(|_| ()),
                (
                    count(hex, 8),
                    count((char('-'), count(hex, 4)), 3),
                    char('-'),
                    count(hex, 12),
                )
                    .map(|_| ()),
            )),
            char('.'),
        )),
        alt((
            alt((
                parse_version.map(|_| ContactAttribute::None),
                parse_prodid.map(|_| ContactAttribute::None),
                parse_display_name.map(ContactAttribute::DisplayName),
                parse_nickname.map(ContactAttribute::Nickname),
                parse_name.map(ContactAttribute::Name),
                parse_phone.map(ContactAttribute::Phone),
                parse_email.map(ContactAttribute::Email),
                parse_address.map(ContactAttribute::Address),
                parse_birthday.map(ContactAttribute::Birthday),
                parse_anniversary.map(ContactAttribute::Anniversary),
                parse_photo.map(ContactAttribute::Photo),
                parse_revision.map(ContactAttribute::Revision),
                parse_uid.map(ContactAttribute::Uid),
                parse_org.map(ContactAttribute::Org),
                parse_title.map(ContactAttribute::Title),
                parse_url.map(ContactAttribute::Url),
                parse_note.map(ContactAttribute::Note),
                parse_attachment.map(ContactAttribute::Attachment),
            )),
            alt((
                preceded(tag("PROFILE"), remaining()).map(|_| ContactAttribute::None),
                preceded(tag("CLASS"), remaining()).map(|_| ContactAttribute::None),
                preceded(tag("SORT-STRING"), remaining()).map(|_| ContactAttribute::None),
                preceded(tag("IMPP"), remaining()).map(|_| ContactAttribute::None),
                preceded(tag("CATEGORIES"), remaining()).map(|_| ContactAttribute::None),
                preceded(tag("X-"), remaining()).map(|_| ContactAttribute::None),
                preceded(tag("VND-63-SENSITIVE-CONTENT-CONFIG"), remaining())
                    .map(|_| ContactAttribute::None),
            )),
        )),
    )
    .parse(attribute_raw.as_ref())
    {
        Ok((_, parsed)) => Ok((rest, parsed)),
        Err(err) => Err(match err {
            nom::Err::Error(err) => nom::Err::Error(nom::error::Error::new(&input, err.code)),
            nom::Err::Failure(err) => nom::Err::Failure(nom::error::Error::new(&input, err.code)),
            nom::Err::Incomplete(err) => nom::Err::Incomplete(err),
        }),
    }
}

fn hex(input: &str) -> IResult<&str, char> {
    one_of("1234567890abcdefABCDEF").parse(input)
}

fn parse_version(input: &str) -> IResult<&str, &str> {
    // preceded(tag("VERSION:"), take_while(char::is_alphanumeric)).parse(input)
    preceded(tag("VERSION:"), remaining()).parse(input)
}

fn parse_prodid(input: &str) -> IResult<&str, &str> {
    preceded(tag("PRODID:"), remaining()).parse(input)
}

fn parse_display_name(input: &str) -> IResult<&str, String> {
    let (rest, display_name) = preceded(tag("FN:"), remaining()).parse(input)?;

    let display_name = display_name.replace("\\", "");

    Ok((rest, display_name.to_string()))
}

fn parse_nickname(input: &str) -> IResult<&str, String> {
    preceded(tag("NICKNAME:"), parse_string).parse(input)
}

fn parse_name(input: &str) -> IResult<&str, Name> {
    let (rest, list) = preceded(
        tag("N:"),
        separated_list0(char(';'), opt(take_while(|c| c != ';'))),
    )
    .parse(input)?;

    let family_name = if let Some(family_name) = list
        .get(0)
        .map(|opt| *opt)
        .flatten()
        .map(|input| parse_string(input))
    {
        Some(family_name?.1)
    } else {
        None
    };

    let given_name = if let Some(given_name) = list
        .get(1)
        .map(|opt| *opt)
        .flatten()
        .map(|input| parse_string(input))
    {
        Some(given_name?.1)
    } else {
        None
    };

    let additional_names = if let Some(additional_names) = list
        .get(2)
        .map(|opt| *opt)
        .flatten()
        .map(|input| opt(separated_list0(char(','), is_not(",;\r"))).parse(input))
    {
        additional_names?
            .1
            .into_iter()
            .flatten()
            .map(|name| name.to_string())
            .collect()
    } else {
        Vec::new()
    };

    let honorific_prefixes = if let Some(prefixes) = list
        .get(2)
        .map(|opt| *opt)
        .flatten()
        .map(|input| opt(separated_list0(char(','), is_not(",;\r"))).parse(input))
    {
        prefixes?
            .1
            .into_iter()
            .flatten()
            .map(|name| name.to_string())
            .collect()
    } else {
        Vec::new()
    };

    let honorific_suffixes = if let Some(suffixes) = list
        .get(2)
        .map(|opt| *opt)
        .flatten()
        .map(|input| opt(separated_list0(char(','), is_not(",;\r"))).parse(input))
    {
        suffixes?
            .1
            .into_iter()
            .flatten()
            .map(|name| name.to_string())
            .collect()
    } else {
        Vec::new()
    };

    Ok((
        rest,
        Name {
            family_name,
            given_name,
            additional_names,
            honorific_prefixes,
            honorific_suffixes,
        },
    ))
}

fn parse_phone(input: &str) -> IResult<&str, Phone> {
    let (rest, (attributes, number)) = preceded(
        tag("TEL"),
        (terminated(parse_attribute_info, char(':')), remaining()),
    )
    .parse(input)?;

    let mut phone = Phone {
        number: number.to_string(),
        phone_type: PhoneType::default(),
    };

    let mut types = Vec::new();

    for attr in attributes {
        match attr {
            AttributeInfo::Type(t) => {
                for value in t {
                    if let Ok(phone_type) = PhoneType::from_str(value) {
                        types.push(phone_type);
                    }
                }
            }
            _ => {}
        }
    }

    if types.len() > 1 {
        if types.contains(&PhoneType::Fax) {
            phone.phone_type = PhoneType::Fax;
        } else {
            phone.phone_type = types.into_iter().next().unwrap();
        }
    } else {
        if let Some(phone_type) = types.into_iter().next() {
            phone.phone_type = phone_type;
        }
    }

    Ok((rest, phone))
}

fn parse_email(input: &str) -> IResult<&str, Email> {
    let (rest, (attributes, email)) = preceded(
        tag("EMAIL"),
        (terminated(parse_attribute_info, char(':')), remaining()),
    )
    .parse(input)?;

    let mut email = Email {
        email: email.to_string(),
        email_type: EmailType::default(),
    };

    for attr in attributes {
        match attr {
            AttributeInfo::Type(t) => {
                for value in t {
                    if let Ok(email_type) = EmailType::from_str(value) {
                        email.email_type = email_type;
                    }
                }
            }
            _ => {}
        }
    }

    Ok((rest, email))
}

fn parse_attribute_info<'a>(input: &'a str) -> IResult<&'a str, Vec<AttributeInfo<'a>>> {
    let (rest, list) = opt(preceded(
        char(';'),
        separated_list1(char(';'), parse_attribute_info_part),
    ))
    .parse(input)?;

    Ok((rest, list.unwrap_or_default()))
}

#[allow(dead_code)]
#[derive(Debug)]
enum AttributeInfo<'a> {
    None,
    Type(Vec<&'a str>),
    Value(&'a str),
    Encoding(&'a str),
    ClipRect(&'a str),
    Pref(u8),
    FmType(&'a str),
}

fn parse_attribute_info_part<'a>(input: &'a str) -> IResult<&'a str, AttributeInfo<'a>> {
    alt((
        preceded(tag("TYPE="), separated_list1(char(','), is_not(",;:"))).map(AttributeInfo::Type),
        preceded(tag("VALUE="), is_not(";:")).map(AttributeInfo::Value),
        preceded(
            tag("PREF="),
            map_res(is_not(";:"), |s: &str| s.parse::<u8>()),
        )
        .map(AttributeInfo::Pref),
        preceded(tag("ENCODING="), is_not(";:")).map(AttributeInfo::Encoding),
        preceded(tag("FMTTYPE="), is_not(";:")).map(AttributeInfo::FmType),
        preceded(tag("X-ABCROP-RECTANGLE="), is_not(";:")).map(AttributeInfo::ClipRect),
        preceded(tag("X-"), is_not(";:")).map(|_| AttributeInfo::None),
    ))
    .parse(input)
}

fn parse_address(input: &str) -> IResult<&str, Address> {
    let (rest, (attributes, list)) = preceded(
        alt((tag("ADR"), tag("ITEM1.ADR"))),
        (
            terminated(parse_attribute_info, char(':')),
            separated_list1(char(';'), opt(is_not("\r;"))),
        ),
    )
    .parse(input)?;

    let post_box = list.get(0).map(|s| *s).flatten().map(|s| s.to_string());
    let extension = list.get(1).map(|s| *s).flatten().map(|s| s.to_string());
    let street = list.get(2).map(|s| *s).flatten().map(|s| s.to_string());
    let locality = list.get(3).map(|s| *s).flatten().map(|s| s.to_string());
    let region = list.get(4).map(|s| *s).flatten().map(|s| s.to_string());
    let postal_code = list.get(5).map(|s| *s).flatten().map(|s| s.to_string());
    let country = list.get(6).map(|s| *s).flatten().map(|s| s.to_string());

    let mut address = Address {
        address_type: AddressType::Home,
        post_box,
        extension,
        street,
        locality,
        region,
        postal_code,
        country,
        ..Default::default()
    };

    for attr in attributes {
        match attr {
            AttributeInfo::Type(t) => {
                for value in t {
                    if let Ok(address_type) = AddressType::from_str(value) {
                        address.address_type = address_type;
                    }
                }
            }
            _ => {}
        }
    }

    Ok((rest, address))
}

fn parse_birthday(input: &str) -> IResult<&str, NaiveDate> {
    let (rest, (_attributes, date)) = preceded(
        tag("BDAY"),
        (terminated(parse_attribute_info, char(':')), parse_date),
    )
    .parse(input)?;

    Ok((rest, date))
}

fn parse_anniversary(input: &str) -> IResult<&str, NaiveDate> {
    let (rest, (_attributes, date)) = preceded(
        tag("ANNIVERSARY"),
        (terminated(parse_attribute_info, char(':')), parse_date),
    )
    .parse(input)?;

    Ok((rest, date))
}

fn parse_photo(input: &str) -> IResult<&str, ContactPhoto> {
    let (rest, (_attributes, data)) = preceded(
        tag("PHOTO"),
        (terminated(parse_attribute_info, char(':')), remaining()),
    )
    .parse(input)?;

    let decoded = base64::prelude::BASE64_STANDARD
        .decode(data)
        .map_err(|_err| {
            nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Fail))
        })?;

    let photo = ContactPhoto { data: decoded };

    Ok((rest, photo))
}

fn parse_attachment(input: &str) -> IResult<&str, Attachment> {
    let (rest, (_attributes, data)) = preceded(
        tag("ATTACH"),
        (terminated(parse_attribute_info, char(':')), remaining()),
    )
    .parse(input)?;

    let decoded = base64::prelude::BASE64_STANDARD
        .decode(data)
        .map_err(|_err| {
            nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Fail))
        })?;

    let photo = Attachment { data: decoded };

    Ok((rest, photo))
}

fn parse_revision(input: &str) -> IResult<&str, NaiveDateTime> {
    preceded(tag("REV:"), parse_datetime).parse(input)
}

fn parse_uid(input: &str) -> IResult<&str, String> {
    let (rest, uid) = preceded(tag("UID:"), remaining()).parse(input)?;

    Ok((rest, uid.to_string()))
}

fn parse_org(input: &str) -> IResult<&str, String> {
    let (rest, raw_org) = preceded(tag("ORG:"), remaining()).parse(input)?;

    let org = String::from(raw_org);

    Ok((rest, org))
}

fn parse_title(input: &str) -> IResult<&str, String> {
    let (rest, raw_org) = preceded(tag("TITLE:"), remaining()).parse(input)?;

    let org = String::from(raw_org);

    Ok((rest, org))
}

fn parse_url(input: &str) -> IResult<&str, Url> {
    let (rest, (attributes, url)) = preceded(
        tag("URL"),
        (
            terminated(parse_attribute_info, char(':')),
            take_while(char::is_alphanumeric),
        ),
    )
    .parse(input)?;

    let mut url = Url {
        url: String::from(url),
        url_type: UrlType::default(),
    };

    for attr in attributes {
        match attr {
            AttributeInfo::Type(t) => {
                for value in t {
                    if let Ok(url_type) = UrlType::from_str(value) {
                        url.url_type = url_type;
                    }
                }
            }
            _ => {}
        }
    }

    Ok((rest, url))
}

fn parse_note(input: &str) -> IResult<&str, String> {
    preceded(tag("NOTE:"), parse_string).parse(input)
}
