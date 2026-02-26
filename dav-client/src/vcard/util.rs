use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::is_not,
    character::complete::{char, one_of},
    combinator::{map, map_res, opt, value, verify},
    multi::{count, fold_many0},
    sequence::{preceded, terminated},
};

fn parse_escaped_char(input: &str) -> IResult<&str, char> {
    preceded(
        char('\\'),
        alt((
            value('\\', char('\\')),
            value('"', char('"')),
            value('\n', char('\n')),
        )),
    )
    .parse(input)
}

fn parse_unescaped_sequence(input: &str) -> IResult<&str, &str> {
    let not_quoted = is_not("\\");

    verify(not_quoted, |s: &str| !s.is_empty()).parse(input)
}

enum StringPart<'a> {
    Literal(&'a str),
    Escaped(char),
}

fn parse_string_part<'a>(input: &'a str) -> IResult<&'a str, StringPart<'a>> {
    alt((
        map(parse_unescaped_sequence, StringPart::Literal),
        map(parse_escaped_char, StringPart::Escaped),
    ))
    .parse(input)
}

pub fn parse_string(input: &str) -> IResult<&str, String> {
    fold_many0(parse_string_part, String::new, |mut string, fragment| {
        match fragment {
            StringPart::Literal(literal) => string.push_str(literal),
            StringPart::Escaped(char) => string.push(char),
        };

        string
    })
    .parse(input)
}

pub fn parse_digits<'a>(
    digits: usize,
) -> impl Parser<&'a str, Output = u32, Error = nom::error::Error<&'a str>> {
    const RADIX: u32 = 10;
    count(one_of("1234567890"), digits).map(|chars| {
        chars
            .into_iter()
            .map(|c| c.to_digit(RADIX).unwrap())
            .fold(0, |ans, i| ans * RADIX + i)
    })
}

pub fn parse_date(input: &str) -> IResult<&str, NaiveDate> {
    map_res(
        (
            parse_digits(4),
            preceded(opt(char('-')), parse_digits(2)),
            preceded(opt(char('-')), parse_digits(2)),
        ),
        |(year, month, day)| {
            NaiveDate::from_ymd_opt(year as i32, month, day)
                .ok_or_else(|| nom::error::Error::new(input, nom::error::ErrorKind::Fail))
        },
    )
    .parse(input)
}

pub fn parse_time(input: &str) -> IResult<&str, NaiveTime> {
    map_res(
        (
            parse_digits(2),
            preceded(opt(char(':')), parse_digits(2)),
            preceded(opt(char(':')), parse_digits(2)),
        ),
        |(hour, minutes, seconds)| {
            NaiveTime::from_hms_opt(hour, minutes, seconds)
                .ok_or_else(|| nom::error::Error::new(input, nom::error::ErrorKind::Fail))
        },
    )
    .parse(input)
}

pub fn parse_datetime(input: &str) -> IResult<&str, NaiveDateTime> {
    (
        terminated(parse_date, opt(char('T'))),
        terminated(parse_time, opt(char('Z'))),
    )
        .map(|(date, time)| NaiveDateTime::new(date, time))
        .parse(input)
}
