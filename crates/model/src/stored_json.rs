//! The forms a record's values take in every document and contract: a time, a duration, and JSON another system
//! defines. A document is written by `serde_json` (ADR-0107): it reads every document an earlier build wrote, Go's
//! escapes and float form included.

use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::macros::{datetime, format_description};
use time::{OffsetDateTime, UtcOffset};

use crate::Invalid;

/// A time as every document and contract holds it: RFC 3339 text with the offset it was written with kept and the
/// fraction's trailing zeros trimmed. Two times are equal, and ordered, as instants. A time that may be unset is an
/// `Option<Time>`. The default, the first instant of year 1 in UTC, is what a record built with `..Default::default()`
/// holds until it is given its times, never a time that means "none".
///
/// A time is always written: one RFC 3339 cannot hold (a year before 0, an offset with seconds, which no clock and no
/// read yields, only arithmetic past every real date) is written as the same instant in UTC, or as the nearest end of
/// RFC 3339's years. A read takes RFC 3339 only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct Time(pub OffsetDateTime);

impl Default for Time {
    fn default() -> Self {
        Self(datetime!(0001-01-01 0:00 UTC))
    }
}

impl From<OffsetDateTime> for Time {
    fn from(at: OffsetDateTime) -> Self {
        Self(at)
    }
}

// A tool input reads a time as an RFC 3339 string.
impl schemars::JsonSchema for Time {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Time".into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string", "format": "date-time" })
    }
}

impl From<Time> for String {
    fn from(at: Time) -> Self {
        at.rfc3339()
    }
}

impl TryFrom<String> for Time {
    type Error = Invalid;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse_rfc3339(&text).ok_or_else(|| Invalid::new(format!("{text:?} is not an RFC 3339 time")))
    }
}

impl Time {
    /// The time as an RFC 3339 string, as it is written everywhere it is sent or stored.
    pub fn rfc3339(&self) -> String {
        if let Ok(text) = self.0.format(&Rfc3339) {
            return text;
        }
        match self.0.checked_to_offset(UtcOffset::UTC).map(|utc| utc.format(&Rfc3339)) {
            Some(Ok(text)) => text,
            _ if self.0.year() < 0 => "0000-01-01T00:00:00Z".to_owned(),
            _ => "9999-12-31T23:59:59.999999999Z".to_owned(),
        }
    }

    pub(crate) fn parse_rfc3339(text: &str) -> Option<Self> {
        OffsetDateTime::parse(text, &Rfc3339).ok().map(Self)
    }

    /// A bare date (2026-12-01) as its first instant in UTC.
    pub fn parse_date(text: &str) -> Option<Self> {
        let date = time::Date::parse(text, format_description!("[year]-[month]-[day]")).ok()?;
        Some(Self(date.midnight().assume_utc()))
    }
}

/// A duration: a count of nanoseconds, signed, written as that integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(into = "i64", from = "i64")]
pub struct Duration(pub i64);

impl From<i64> for Duration {
    fn from(nanos: i64) -> Self {
        Self(nanos)
    }
}

impl From<Duration> for i64 {
    fn from(d: Duration) -> Self {
        d.0
    }
}

impl Duration {
    pub const ZERO: Duration = Duration(0);
    pub const SECOND: Duration = Duration(1_000_000_000);
    pub const MINUTE: Duration = Duration(60 * Self::SECOND.0);
    pub const HOUR: Duration = Duration(60 * Self::MINUTE.0);

    /// `n` minutes; the longest duration when that is past what the count holds.
    pub const fn minutes(n: i64) -> Duration {
        Duration(n.saturating_mul(Self::MINUTE.0))
    }
}

/// JSON another system defines (a plugin's JSON Schema, a tool's arguments, a reading's value, a script's result), carried
/// as it is. A readable format writes the JSON itself; postcard, which cannot carry a value that does not say its type,
/// writes it as JSON text (ADR-0095). A `null` it holds is the value `null`; a field with none is an `Option` of it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ForeignJson(pub serde_json::Value);

impl ForeignJson {
    /// `text` read as one JSON value.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text).map(Self)
    }

    /// The value as compact JSON text.
    pub fn text(&self) -> String {
        self.0.to_string()
    }

    /// The value of key, when the JSON is an object that holds it.
    pub fn field(&self, key: &str) -> Option<&serde_json::Value> {
        self.0.get(key)
    }

    /// data with key set to value beside the keys it holds; data that is no object is replaced by one holding key alone.
    pub fn with_field(data: Option<&ForeignJson>, key: &str, value: serde_json::Value) -> ForeignJson {
        let mut fields = match data.map(|d| &d.0) {
            Some(serde_json::Value::Object(fields)) => fields.clone(),
            _ => serde_json::Map::new(),
        };
        fields.insert(key.to_owned(), value);
        ForeignJson(serde_json::Value::Object(fields))
    }
}

impl Serialize for ForeignJson {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            self.0.serialize(serializer)
        } else {
            serializer.serialize_str(&self.text())
        }
    }
}

impl<'de> Deserialize<'de> for ForeignJson {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            serde_json::Value::deserialize(deserializer).map(Self)
        } else {
            let text = String::deserialize(deserializer)?;
            Self::parse(&text).map_err(serde::de::Error::custom)
        }
    }
}

impl From<serde_json::Value> for ForeignJson {
    fn from(value: serde_json::Value) -> Self {
        Self(value)
    }
}

/// A JSON object another system defines (a tool call's input), carried in the form [`ForeignJson`] takes: the object in a
/// readable format, its JSON text in postcard. For a field `#[serde(with = "model::stored_json::foreign_object")]`.
pub mod foreign_object {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use serde_json::{Map, Value};

    pub fn serialize<S: Serializer>(object: &Map<String, Value>, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            object.serialize(serializer)
        } else {
            let text = serde_json::to_string(object).map_err(serde::ser::Error::custom)?;
            serializer.serialize_str(&text)
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Map<String, Value>, D::Error> {
        if deserializer.is_human_readable() {
            Map::deserialize(deserializer)
        } else {
            let text = String::deserialize(deserializer)?;
            serde_json::from_str(&text).map_err(serde::de::Error::custom)
        }
    }
}
