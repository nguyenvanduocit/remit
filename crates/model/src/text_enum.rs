//! String enums (a stored text from a fixed set, such as `ChainStatus`) as Rust enums that read only the texts they name.

use std::fmt;

/// A text that names no value of the enum it was read as: a value of a newer build, or a word an agent made up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownValue {
    /// The enum's name.
    pub name: &'static str,
    pub text: String,
    /// What the enum names.
    pub values: &'static [&'static str],
}

impl fmt::Display for UnknownValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} is not a {}: it is one of {}",
            self.text,
            self.name,
            self.values.join(", ")
        )
    }
}

impl std::error::Error for UnknownValue {}

/// Declares a string enum: a variant for each value, with its text. It reads and writes as its text and reads no other,
/// so a field whose value may be absent is an `Option` of it, and a stored text this build does not name fails the read of
/// the record that holds it (the store leaves that record out, ADR-0107). An enum whose empty text is a value names it
/// (`Neither = ""`). One that a record needs a default of derives `Default` and marks that value `#[default]`. Any crate
/// declares one with it, and tsgen writes it as the union of its texts.
#[macro_export]
macro_rules! text_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $( $(#[$variant_meta:meta])* $variant:ident = $text:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $( $(#[$variant_meta])* $variant, )+
        }

        impl $name {
            /// Every value's text: what a contract that carries this enum must accept.
            pub const VALUES: &'static [&'static str] = &[$( $text ),+];

            /// The value as it is stored.
            pub const fn as_str(&self) -> &'static str {
                match self {
                    $( Self::$variant => $text, )+
                }
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = $crate::UnknownValue;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                match text {
                    $( $text => Ok(Self::$variant), )+
                    other => Err($crate::UnknownValue {
                        name: stringify!($name),
                        text: other.to_owned(),
                        values: Self::VALUES,
                    }),
                }
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        // A tool input names one of its values: the schema lists them, so another is refused before it is read.
        impl $crate::__schemars::JsonSchema for $name {
            fn schema_name() -> ::std::borrow::Cow<'static, str> {
                stringify!($name).into()
            }

            fn inline_schema() -> bool {
                true
            }

            fn json_schema(_: &mut $crate::__schemars::SchemaGenerator) -> $crate::__schemars::Schema {
                $crate::__schemars::json_schema!({ "type": "string", "enum": Self::VALUES })
            }
        }

        impl $crate::__serde::Serialize for $name {
            fn serialize<S: $crate::__serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> $crate::__serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::__serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = <::std::borrow::Cow<'de, str> as $crate::__serde::Deserialize>::deserialize(deserializer)?;
                text.parse().map_err(|_| {
                    <D::Error as $crate::__serde::de::Error>::unknown_variant(&text, Self::VALUES)
                })
            }
        }
    };
}
