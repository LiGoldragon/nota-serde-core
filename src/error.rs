use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Custom(String),

    #[error("unit type `()` is forbidden in nota — use a named variant like `Nil` or `None` for absent-value semantics")]
    UnitForbidden,

    #[error("string literal cannot contain `|]` (would close a multiline string); no escape syntax exists yet")]
    StringContainsMultilineCloser,

    #[error("non-finite floats (NaN, +inf, -inf) have no nota representation")]
    NonFiniteFloat,

    #[error("serialize_value called before serialize_key in map")]
    MapValueWithoutKey,

    #[error("multi-field unnamed struct `{name}` (len {len}) — nota requires named-field structs; use `struct {name} {{ … }}` instead of `struct {name}(…, …)`")]
    MultiFieldTupleStructForbidden {
        name: &'static str,
        len: usize,
    },
}

impl serde::ser::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }
}

impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::Custom(msg.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
