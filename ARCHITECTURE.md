# ARCHITECTURE — nota-serde-core

The shared lexer + ser/de kernel for every nota-family grammar.
Parameterised by `Dialect`: nota, nexus, future grammars. Used
by:

- [nota-serde](https://github.com/LiGoldragon/nota-serde) — the
  public façade for nota text.
- [nexus-serde](https://github.com/LiGoldragon/nexus-serde) —
  the public façade for nexus text.
- [nexusd](https://github.com/LiGoldragon/nexusd) — parses
  incoming nexus text at `Dialect::Nexus` to build signal
  frames.

This crate is the **internal kernel**. End users link
`nota-serde` or `nexus-serde`, never this directly.

## Boundaries

Owns:

- Tokeniser: produces a token stream from raw bytes.
- AST shape: shared structure for parsed nota-family input.
- Dialect parameter: a token-level switch per grammar
  variant.
- Serde façade: serializing + deserializing Rust types via the
  `serde::{Serialize, Deserialize}` traits, going through the
  AST.

Does not own:

- Grammar semantics (those live in the per-dialect façade
  crates and in the language repos: [nota](https://github.com/LiGoldragon/nota),
  [nexus](https://github.com/LiGoldragon/nexus)).
- Type-level mappings to specific record kinds — that's a
  consumer's concern.

## Code map

```
src/
├── lib.rs   — entry, public surface
├── lex.rs   — tokenisation
├── ast.rs   — parsed-tree types
├── parse.rs — token → AST
├── ser.rs   — Rust value → AST → text
└── de.rs    — text → AST → Rust value
```

## Status

CANON. End-user surface is via the façades.

## Cross-cutting context

- nota grammar: [github.com/LiGoldragon/nota](https://github.com/LiGoldragon/nota)
- nexus grammar:
  [github.com/LiGoldragon/nexus](https://github.com/LiGoldragon/nexus)
- Layer 0 of the project architecture:
  [criome/ARCHITECTURE.md §8](https://github.com/LiGoldragon/criome/blob/main/ARCHITECTURE.md)
