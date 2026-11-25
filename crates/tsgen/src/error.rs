use std::fmt;

use proc_macro2::Span;

/// Why the generator stopped: the wire source holds something it does not write, at `file:line`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub file: String,
    pub line: usize,
    pub message: String,
}

impl Error {
    pub fn new(file: &str, line: usize, message: impl Into<String>) -> Self {
        Self {
            file: file.to_owned(),
            line,
            message: message.into(),
        }
    }

    pub fn at(file: &str, span: Span, message: impl Into<String>) -> Self {
        Self::new(file, span.start().line, message)
    }

    pub fn syn(file: &str, error: &syn::Error) -> Self {
        Self::at(file, error.span(), error.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.file, self.line, self.message)
    }
}

impl std::error::Error for Error {}
