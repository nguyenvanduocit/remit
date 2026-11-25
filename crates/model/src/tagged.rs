//! Shapes a readable format writes that postcard cannot read, and the macros that write a type's `Serialize` and
//! `Deserialize` in two forms: the derived one for postcard, and the readable one moved from it (ADR-0095). Postcard does
//! not describe itself, so it reads neither an internally tagged enum nor a flattened field; such a type derives its
//! externally tagged or nested form with `#[serde(remote = "Self")]`, and the macro moves it for a readable format.
//! `tsgen` reads each macro, so the generated TypeScript is the readable form.
//!
//! - `json_tagged!(Enum, "field")`: an enum with its variant in `field` (`{"kind": "owner", …}`).
//! - `json_paired!(Struct, field, "tag", "content")`: a struct's enum field written as two of the struct's own keys, the
//!   variant in `tag` and the text it holds in `content` (`{"kind": "tool", "status": "completed", …}`); a variant that
//!   holds nothing has `content` empty.

use serde_json::{Map, Value};

/// The externally tagged JSON of a variant (`"name"` or `{"name": {…fields}}`) with the variant in `tag` instead.
pub fn to_internal(tag: &str, external: Value) -> Result<Value, String> {
    match external {
        Value::String(name) => Ok(Value::Object(Map::from_iter([(tag.to_owned(), Value::String(name))]))),
        Value::Object(outer) if outer.len() == 1 => {
            let Some((name, fields)) = outer.into_iter().next() else {
                return Err("an empty variant".to_owned());
            };
            let Value::Object(fields) = fields else {
                return Err(format!("variant {name} holds no fields, so it cannot carry `{tag}`"));
            };
            if fields.contains_key(tag) {
                return Err(format!("variant {name} has a field named `{tag}`, the enum's tag"));
            }
            let mut out = Map::with_capacity(fields.len().saturating_add(1));
            out.insert(tag.to_owned(), Value::String(name));
            out.extend(fields);
            Ok(Value::Object(out))
        }
        other => Err(format!("not a variant: {other}")),
    }
}

/// Reads a variant written with its name in `tag`, through `read`, which reads the externally tagged form. A variant
/// with no field left is a unit variant, or a struct variant whose fields all have defaults.
pub fn from_internal<T>(
    tag: &str,
    internal: Value,
    read: impl Fn(Value) -> Result<T, serde_json::Error>,
) -> Result<T, String> {
    let Value::Object(mut fields) = internal else {
        return Err(format!("expected an object with `{tag}`"));
    };
    let Some(Value::String(name)) = fields.remove(tag) else {
        return Err(format!("missing field `{tag}`"));
    };
    if fields.is_empty()
        && let Ok(unit) = read(Value::String(name.clone()))
    {
        return Ok(unit);
    }
    read(Value::Object(Map::from_iter([(name, Value::Object(fields))]))).map_err(|e| e.to_string())
}

/// A struct's nested JSON (`{…, "field": "name"}` or `{…, "field": {"name": "text"}}`, the externally tagged enum in
/// `field`) with the enum's variant in `tag` and the text it holds in `content` instead, empty for a variant that holds
/// none.
pub fn to_paired(field: &str, tag: &str, content: &str, nested: Value) -> Result<Value, String> {
    let Value::Object(mut fields) = nested else {
        return Err(format!("expected an object with `{field}`"));
    };
    let (name, text) = match fields.remove(field) {
        Some(Value::String(name)) => (name, String::new()),
        Some(Value::Object(outer)) if outer.len() == 1 => match outer.into_iter().next() {
            Some((name, Value::String(text))) => (name, text),
            _ => return Err(format!("`{field}` holds more than a text")),
        },
        other => return Err(format!("`{field}` is no variant: {other:?}")),
    };
    if fields.contains_key(tag) || fields.contains_key(content) {
        return Err(format!(
            "a field is named `{tag}` or `{content}`, which `{field}` is written as"
        ));
    }
    fields.insert(tag.to_owned(), Value::String(name));
    fields.insert(content.to_owned(), Value::String(text));
    Ok(Value::Object(fields))
}

/// Reads a struct written with the variant of its enum `field` in `tag` and its text in `content`, through `read`, which
/// reads the nested form. An empty `content` is the variant that holds nothing, else the one that holds the empty text.
/// A key left out reads as empty, as a record's missing key reads as its default.
pub fn from_paired<T>(
    field: &str,
    tag: &str,
    content: &str,
    paired: Value,
    read: impl Fn(&Value) -> Result<T, serde_json::Error>,
) -> Result<T, String> {
    let Value::Object(mut fields) = paired else {
        return Err(format!("expected an object with `{tag}`"));
    };
    let text = |v: Option<Value>| match v {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(s)) => Ok(s),
        Some(other) => Err(format!("not a text: {other}")),
    };
    let name = text(fields.remove(tag))?;
    let held = text(fields.remove(content))?;
    if held.is_empty() {
        fields.insert(field.to_owned(), Value::String(name.clone()));
        let unit = Value::Object(fields);
        if let Ok(read) = read(&unit) {
            return Ok(read);
        }
        let Value::Object(back) = unit else {
            return Err(format!("expected an object with `{tag}`"));
        };
        fields = back;
    }
    let variant = Map::from_iter([(name, Value::String(held))]);
    fields.insert(field.to_owned(), Value::Object(variant));
    read(&Value::Object(fields)).map_err(|e| e.to_string())
}

/// `Serialize` and `Deserialize` for an enum that derives them with `#[serde(remote = "Self")]`, externally tagged:
/// postcard reads that form, a readable format the variant in `$tag`.
#[macro_export]
macro_rules! json_tagged {
    ($ty:ident, $tag:literal) => {
        impl $crate::__serde::Serialize for $ty {
            fn serialize<S: $crate::__serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                if !serializer.is_human_readable() {
                    return $ty::serialize(self, serializer);
                }
                let external = $ty::serialize(self, $crate::__serde_json::value::Serializer)
                    .map_err(<S::Error as $crate::__serde::ser::Error>::custom)?;
                let internal = $crate::tagged::to_internal($tag, external)
                    .map_err(<S::Error as $crate::__serde::ser::Error>::custom)?;
                $crate::__serde::Serialize::serialize(&internal, serializer)
            }
        }

        impl<'de> $crate::__serde::Deserialize<'de> for $ty {
            fn deserialize<D: $crate::__serde::Deserializer<'de>>(
                deserializer: D,
            ) -> ::core::result::Result<Self, D::Error> {
                if !deserializer.is_human_readable() {
                    return $ty::deserialize(deserializer);
                }
                let internal =
                    <$crate::__serde_json::Value as $crate::__serde::Deserialize>::deserialize(deserializer)?;
                $crate::tagged::from_internal($tag, internal, $ty::deserialize)
                    .map_err(<D::Error as $crate::__serde::de::Error>::custom)
            }
        }
    };
}

/// `Serialize` and `Deserialize` for a struct that derives them with `#[serde(remote = "Self")]`, its enum field `$field`
/// externally tagged: postcard reads that form, a readable format the variant in `$tag` and the text it holds in
/// `$content`, two keys of the struct's own.
#[macro_export]
macro_rules! json_paired {
    ($ty:ident, $field:ident, $tag:literal, $content:literal) => {
        impl $crate::__serde::Serialize for $ty {
            fn serialize<S: $crate::__serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                if !serializer.is_human_readable() {
                    return $ty::serialize(self, serializer);
                }
                let nested = $ty::serialize(self, $crate::__serde_json::value::Serializer)
                    .map_err(<S::Error as $crate::__serde::ser::Error>::custom)?;
                let paired = $crate::tagged::to_paired(stringify!($field), $tag, $content, nested)
                    .map_err(<S::Error as $crate::__serde::ser::Error>::custom)?;
                $crate::__serde::Serialize::serialize(&paired, serializer)
            }
        }

        impl<'de> $crate::__serde::Deserialize<'de> for $ty {
            fn deserialize<D: $crate::__serde::Deserializer<'de>>(
                deserializer: D,
            ) -> ::core::result::Result<Self, D::Error> {
                if !deserializer.is_human_readable() {
                    return $ty::deserialize(deserializer);
                }
                let paired = <$crate::__serde_json::Value as $crate::__serde::Deserialize>::deserialize(deserializer)?;
                $crate::tagged::from_paired(stringify!($field), $tag, $content, paired, |v| {
                    $ty::deserialize(v)
                })
                .map_err(<D::Error as $crate::__serde::de::Error>::custom)
            }
        }
    };
}
