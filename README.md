# nota-serde-core

Shared kernel for [nota-serde](https://github.com/LiGoldragon/nota-serde)
and [nexus-serde](https://github.com/LiGoldragon/nexus-serde).

Holds the format machinery both crates need:

- `lexer::{Lexer, Token}` — tokenisation of nota/nexus text.
- `ser::Serializer` — canonical nota text output.
- `de::Deserializer` — nota text parsing.
- `error::{Error, Result}` — shared error type.

The public API is **internal-facing** — it exists to serve
`nota-serde` and `nexus-serde`, not end-users directly. Use those
crates for application-level serialisation.

## When to reach for this crate

You're building a third nota-grammar-family format (hypothetically
a binary wire form, or a nexus superset). Depend on this crate,
wrap `Serializer` / `Deserializer` with your own façade, add
sentinel dispatch for your format-specific types.

Otherwise: use `nota-serde` (data format) or `nexus-serde`
(messaging protocol).

## License

[License of Non-Authority](LICENSE.md).
