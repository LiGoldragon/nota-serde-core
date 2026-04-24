//! nota-serde-core — shared kernel for [nota-serde](https://github.com/LiGoldragon/nota-serde)
//! and [nexus-serde](https://github.com/LiGoldragon/nexus-serde).
//!
//! Holds the format machinery that both crates need:
//!
//! - [`lexer::Lexer`] and [`lexer::Token`] — tokenise nota/nexus text.
//! - [`ser::Serializer`] — serde `Serializer` producing canonical nota
//!   text (positional records, bare ident-shaped strings, sorted maps,
//!   shortest-roundtrip numbers).
//! - [`de::Deserializer`] — serde `Deserializer` consuming nota text.
//! - [`error::Error`] — unified error type.
//!
//! Consumers (`nota-serde`, `nexus-serde`) wrap these with their
//! own thin façades — `to_string` / `from_str` convenience fns for
//! nota; sentinel-dispatch wrappers for nexus's query-layer types.
//!
//! This crate's public API is **internal-facing** — it evolves with the
//! needs of the two serde crates. Treat breaking changes as minor-bump
//! pre-1.0.
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

pub use de::{from_str, Deserializer};
pub use error::{Error, Result};
pub use ser::{to_string, Serializer};
