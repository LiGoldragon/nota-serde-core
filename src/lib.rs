//! nota-serde-core — shared kernel for
//! [nota-serde](https://github.com/LiGoldragon/nota-serde) and
//! [nexus-serde](https://github.com/LiGoldragon/nexus-serde).
//!
//! Holds the format machinery both crates need:
//!
//! - [`lexer::Lexer`] + [`lexer::Token`] — tokenise nota/nexus text.
//! - [`ser::Serializer`] — serde `Serializer` producing canonical nota
//!   or nexus text (positional records, bare ident-shaped strings,
//!   sorted maps, shortest-roundtrip numbers).
//! - [`de::Deserializer`] — serde `Deserializer`.
//! - [`error::Error`] — unified error type.
//!
//! [`lexer::Dialect`] selects grammar: [`Dialect::Nota`](lexer::Dialect::Nota)
//! is the strict data-layer subset; [`Dialect::Nexus`](lexer::Dialect::Nexus)
//! is the messaging superset (extra delimiters, sigils, sentinel
//! newtype-struct dispatch).
//!
//! Consumers wrap these:
//!
//! - `nota-serde` re-exports the default (Nota) pair.
//! - `nexus-serde` re-exports the `_nexus` pair and adds the three
//!   sentinel wrapper types (`Bind`, `Mutate`, `Negate`).
//!
//! This crate's public API is **internal-facing** — it evolves with
//! the needs of the two serde crates. Treat breaking changes as
//! minor-bump pre-1.0.
//!
//! ```
//! use nota_serde_core::{to_string, from_str};
//!
//! #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
//! struct Point { horizontal: f64, vertical: f64 }
//!
//! let p = Point { horizontal: 3.0, vertical: 4.0 };
//! let text = to_string(&p)?;
//! assert_eq!(text, "(Point 3.0 4.0)");
//! let back: Point = from_str(&text)?;
//! assert_eq!(back, p);
//! # Ok::<(), nota_serde_core::Error>(())
//! ```

pub mod de;
pub mod error;
pub mod lexer;
pub mod ser;

pub use de::{from_str, from_str_nexus, from_str_with, Deserializer};
pub use error::{Error, Result};
pub use lexer::{Dialect, Lexer, Token};
pub use ser::{
    to_string, to_string_nexus, to_string_with, Serializer,
    ATOMIC_BATCH_SENTINEL, BIND_SENTINEL, MUTATE_SENTINEL, NEGATE_SENTINEL,
    SUBSCRIBE_SENTINEL, VALIDATE_SENTINEL,
};
