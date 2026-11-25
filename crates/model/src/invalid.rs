use std::fmt;

/// Why a record is not sound, in words its writer reads: an agent corrects its call from them. The error of the model's
/// checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invalid(String);

impl Invalid {
    pub(crate) fn new(why: impl Into<String>) -> Self {
        Self(why.into())
    }
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Invalid {}
