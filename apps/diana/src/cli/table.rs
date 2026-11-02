//! TSV and JSON presentation of query rows.

use super::output::{Failure, Format, publish};

/// One output record: the canonical locator plus any extra columns.
pub struct Record {
    pub at: String,
    pub value: Option<String>,
    pub entry: Option<String>,
}

impl Record {
    pub fn new(at: String) -> Self {
        Self {
            at,
            value: None,
            entry: None,
        }
    }
    pub fn with_value(mut self, value: String) -> Self {
        self.value = Some(value);
        self
    }
    pub fn with_entry(mut self, entry: String) -> Self {
        self.entry = Some(entry);
        self
    }
}

pub fn publish_records(
    records: &[Record],
    columns: &[&str],
    command: &str,
    format: Format,
    output: Option<&std::path::Path>,
) -> Result<(), Failure> {
    let text = match format {
        Format::Text => {
            let mut text = String::new();
            text.push_str(&columns.join("\t"));
            text.push('\n');
            for record in records {
                let mut fields = Vec::with_capacity(columns.len());
                for column in columns {
                    let value = match *column {
                        "at" => record.at.as_str(),
                        "value" => record.value.as_deref().unwrap_or_default(),
                        "entry" => record.entry.as_deref().unwrap_or_default(),
                        _ => "",
                    };
                    fields.push(escape(value));
                }
                text.push_str(&fields.join("\t"));
                text.push('\n');
            }
            text
        }
        Format::Json => {
            let items: Vec<_> = records
                .iter()
                .map(|record| {
                    let mut object = serde_json::Map::new();
                    for column in columns {
                        let value = match *column {
                            "at" => record.at.clone(),
                            "value" => record.value.clone().unwrap_or_default(),
                            "entry" => record.entry.clone().unwrap_or_default(),
                            _ => String::new(),
                        };
                        object.insert((*column).to_owned(), serde_json::Value::String(value));
                    }
                    serde_json::Value::Object(object)
                })
                .collect();
            serde_json::json!({
                "schema_version": 1,
                "command": command,
                "items": items,
            })
            .to_string()
        }
    };
    publish(output, text.as_bytes(), format)
}

/// Escapes a display field so one record never spans over two lines.
fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '\t' => escaped.push_str("\\t"),
            '\r' => escaped.push_str("\\r"),
            '\n' => escaped.push_str("\\n"),
            '\0' => escaped.push_str("\\0"),
            other => escaped.push(other),
        }
    }
    escaped
}
