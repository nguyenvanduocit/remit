//! The event value a reading carries.

use model::{EventValue, Reading};

/// What event reading r carries; none for a reading that is no event's.
pub fn event_value_of(r: &Reading) -> Option<EventValue> {
    let v: EventValue = serde_json::from_value(r.value.as_ref()?.0.clone()).ok()?;
    (!v.event.via.as_str().is_empty()).then_some(v)
}
