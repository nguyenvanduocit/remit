//! JSON read into a typed value leniently: a `null` leaves the zero value, as a missing member does.

use serde::{Deserialize, Deserializer};

/// The value of a member, or its type's zero (`""`, `0`, `false`, empty) when the member is `null`. Paired with
/// `#[serde(default)]`, a field reads the same whether the member is missing or null.
pub fn null_as_zero<'de, D: Deserializer<'de>, T: Default + Deserialize<'de>>(
    d: D,
) -> std::result::Result<T, D::Error> {
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}
