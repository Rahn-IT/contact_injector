use std::collections::HashMap;

pub mod icalendar;
pub mod vcard;

struct VObject {
    class: String,
    sub_objects: Vec<VObject>,
    properties: Vec<VProperty>,
}

struct VProperty {
    class: String,
    metadata: HashMap<String, String>,
    values: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum VParseError {
    #[error("missing begin")]
    MissingBegin,
    #[error("unexpected end")]
    UnexpectedEnd,
    #[error("end with incorrect class")]
    IncorrectEnd,
    #[error("line is missing colon")]
    MissingColon,
    #[error("line is missing property class")]
    MissingPropertyClass,
    #[error("missing '=' sign in metadata pair")]
    MetadataPairError,
}

impl VObject {
    fn parse(data: &str) -> Result<VObject, VParseError> {
        // TODO: make this more efficent, maybe even zero/minimal copy
        let cleaned_data: &str = &data.trim().replace("\n ", "");
        let mut lines = cleaned_data.lines().peekable();

        let begin = lines.next().ok_or(VParseError::MissingBegin)?;
        if !begin.starts_with("BEGIN:") {
            return Err(VParseError::MissingBegin);
        }

        let class = &begin[6..];
        let mut sub_objects = Vec::new();
        let mut properties = Vec::new();

        loop {
            let next_line = lines.peek().ok_or(VParseError::UnexpectedEnd)?;
            if next_line.starts_with("BEGIN:") {
                let sub_object = VObject::parse(next_line)?;
                sub_objects.push(sub_object);
            }

            let line = lines.next().ok_or(VParseError::UnexpectedEnd)?;

            if line.starts_with("END:") {
                if &line[4..] == class {
                    return Ok(VObject {
                        class: class.to_string(),
                        sub_objects,
                        properties,
                    });
                } else {
                    return Err(VParseError::IncorrectEnd);
                }
            } else {
                let property = VProperty::parse(line)?;
                properties.push(property);
            }
        }
    }

    pub fn get_multi_property(&self, key: &str) -> Vec<&VProperty> {
        self.properties.iter().filter(|p| p.class == key).collect()
    }

    pub fn get_property(&self, key: &str) -> Option<&VProperty> {
        self.properties.iter().find(|p| p.class == key)
    }

    pub fn get_property_value(&self, key: &str) -> Option<&str> {
        let value = self.get_property(key)?.values.first()?.as_str();

        Some(value)
    }
}

impl VProperty {
    fn parse(line: &str) -> Result<VProperty, VParseError> {
        let (metadata, values) = line.split_once(':').ok_or(VParseError::MissingColon)?;

        let mut metadata_pairs = metadata.split(';');
        let mut metadata = HashMap::new();

        let class = metadata_pairs
            .next()
            .ok_or(VParseError::MissingPropertyClass)?;

        for pair in metadata_pairs {
            let (key, value) = pair.split_once('=').ok_or(VParseError::MetadataPairError)?;
            metadata.insert(key.to_string(), value.to_string());
        }

        let values = values.split(',').map(|value| value.to_string()).collect();

        Ok(Self {
            class: class.to_string(),
            metadata,
            values,
        })
    }
}
