//! The records read a stored document and write it back with every value it held: compared as JSON values because key
//! order in a stored document is not stable. A record writes every field, so what it writes holds the document and the
//! fields the document left out (ADR-0107). A document is written by `serde_json`; one an earlier build wrote with Go's
//! escapes and float form reads. The samples are synthetic.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

const STORED_RECORDS: &str = include_str!("fixtures/stored_records.json");

type Check = Result<(), Box<dyn std::error::Error>>;

fn sample(name: &str) -> Result<Value, serde_json::Error> {
    let mut all: Value = serde_json::from_str(STORED_RECORDS)?;
    Ok(all.get_mut(name).map(Value::take).unwrap_or_default())
}

/// `doc` read as T and written again, as JSON text and as the value of that text.
fn round_trip<T: DeserializeOwned + Serialize>(doc: &Value) -> Result<(String, Value), serde_json::Error> {
    let record: T = serde_json::from_value(doc.clone())?;
    let text = serde_json::to_string(&record)?;
    let value = serde_json::from_str(&text)?;
    Ok((text, value))
}

/// Whether `written` holds every value of `doc`: each key of an object with a value that holds it, a list of the same
/// length whose items hold doc's, and any other value equal.
fn holds(written: &Value, doc: &Value) -> bool {
    match (written, doc) {
        (Value::Object(w), Value::Object(d)) => d.iter().all(|(k, v)| w.get(k).is_some_and(|wv| holds(wv, v))),
        (Value::Array(w), Value::Array(d)) => w.len() == d.len() && w.iter().zip(d).all(|(wv, v)| holds(wv, v)),
        _ => written == doc,
    }
}

#[test]
fn a_stored_record_is_written_back_with_every_value_it_held() -> Check {
    for name in ["chain", "landing", "event", "subscription", "account"] {
        let doc = sample(name)?;
        let (_, written) = match name {
            "chain" => round_trip::<model::Chain>(&doc)?,
            "landing" => round_trip::<model::Landing>(&doc)?,
            "event" => round_trip::<model::Event>(&doc)?,
            "subscription" => round_trip::<model::Subscription>(&doc)?,
            _ => round_trip::<model::Account>(&doc)?,
        };
        assert!(holds(&written, &doc), "{name}: {written} does not hold {doc}");
    }
    Ok(())
}
