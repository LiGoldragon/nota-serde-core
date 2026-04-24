//! Nexus-dialect integration tests — lexer extensions, sentinel
//! dispatch on `Bind`/`Mutate`/`Negate`, and Tier-1 delimiter tokens
//! (`<| |>`, `(|| ||)`, `{|| ||}`).

use nota_serde_core::{
    from_str, from_str_nexus, to_string_nexus, to_string,
    Dialect, Error, Lexer, Token,
};
use serde::{Deserialize, Serialize};

fn lex_nexus(s: &str) -> Vec<Token> {
    let mut l = Lexer::nexus(s);
    let mut out = Vec::new();
    while let Some(t) = l.next_token().expect("lex") {
        out.push(t);
    }
    out
}

// ---------------------------------------------------------------------------
// Lexer: nexus-dialect tokens that nota rejects.

mod lexer_nexus_sigils {
    use super::*;

    #[test]
    fn tilde_at_bang_produce_tokens() {
        assert_eq!(lex_nexus("~@!"), vec![Token::Tilde, Token::At, Token::Bang]);
    }

    #[test]
    fn tilde_in_nota_mode_is_error() {
        let mut l = Lexer::new("~");
        assert!(l.next_token().is_err());
    }

    #[test]
    fn at_in_nota_mode_is_error() {
        let mut l = Lexer::new("@foo");
        assert!(l.next_token().is_err());
    }

    #[test]
    fn bang_in_nota_mode_is_error() {
        let mut l = Lexer::new("!x");
        assert!(l.next_token().is_err());
    }

    #[test]
    fn brace_in_nota_mode_is_error() {
        let mut l = Lexer::new("{");
        assert!(l.next_token().is_err());
    }
}

mod lexer_nexus_delimiters {
    use super::*;

    #[test]
    fn braces() {
        assert_eq!(lex_nexus("{}"), vec![Token::LBrace, Token::RBrace]);
    }

    #[test]
    fn constrain_delim() {
        assert_eq!(lex_nexus("{| |}"), vec![Token::LBracePipe, Token::RBracePipe]);
    }

    #[test]
    fn pattern_delim() {
        assert_eq!(lex_nexus("(| |)"), vec![Token::LParenPipe, Token::RParenPipe]);
    }

    #[test]
    fn pattern_delim_with_content() {
        let toks = lex_nexus("(| Point @h |)");
        assert_eq!(toks, vec![
            Token::LParenPipe,
            Token::Ident("Point".into()),
            Token::At,
            Token::Ident("h".into()),
            Token::RParenPipe,
        ]);
    }
}

// ---------------------------------------------------------------------------
// Lexer: Tier-1 tokens (report 013).

mod lexer_tier1 {
    use super::*;

    #[test]
    fn stream_delim_open_close() {
        assert_eq!(lex_nexus("<| |>"), vec![Token::LAnglePipe, Token::RAnglePipe]);
    }

    #[test]
    fn optional_pattern_delim() {
        assert_eq!(
            lex_nexus("(|| ||)"),
            vec![Token::LParenDouble, Token::RParenDouble]
        );
    }

    #[test]
    fn atomic_txn_delim() {
        assert_eq!(
            lex_nexus("{|| ||}"),
            vec![Token::LBraceDouble, Token::RBraceDouble]
        );
    }

    #[test]
    fn windowed_stream_delim() {
        assert_eq!(
            lex_nexus("<|| ||>"),
            vec![Token::LAngleDouble, Token::RAngleDouble]
        );
    }

    #[test]
    fn windowed_stream_disambiguates_from_single() {
        assert_eq!(lex_nexus("<|"), vec![Token::LAnglePipe]);
        assert_eq!(lex_nexus("<||"), vec![Token::LAngleDouble]);
        assert_eq!(lex_nexus("|>"), vec![Token::RAnglePipe]);
        assert_eq!(lex_nexus("||>"), vec![Token::RAngleDouble]);
    }

    #[test]
    fn tier1_vs_non_tier1_disambiguates() {
        // `(|` is pattern; `(||` is optional pattern. Grammar must pick
        // the double-pipe form when the second `|` is present.
        assert_eq!(lex_nexus("(|"), vec![Token::LParenPipe]);
        assert_eq!(lex_nexus("(||"), vec![Token::LParenDouble]);
    }

    #[test]
    fn stream_carrying_pattern() {
        let toks = lex_nexus("<|(| Point |)|>");
        assert_eq!(toks, vec![
            Token::LAnglePipe,
            Token::LParenPipe,
            Token::Ident("Point".into()),
            Token::RParenPipe,
            Token::RAnglePipe,
        ]);
    }

    #[test]
    fn stream_only_in_nexus_mode() {
        // In nota mode, `<|` is LAngle + then `|` is an error.
        let mut l = Lexer::new("<|");
        assert_eq!(l.next_token().unwrap(), Some(Token::LAngle));
        assert!(l.next_token().is_err());
    }

    #[test]
    fn double_close_disambiguates_from_single() {
        assert_eq!(lex_nexus("|)"), vec![Token::RParenPipe]);
        assert_eq!(lex_nexus("||)"), vec![Token::RParenDouble]);
        assert_eq!(lex_nexus("|}"), vec![Token::RBracePipe]);
        assert_eq!(lex_nexus("||}"), vec![Token::RBraceDouble]);
    }

    #[test]
    fn bare_double_pipe_no_closer_errors() {
        let mut l = Lexer::nexus("||x");
        assert!(l.next_token().is_err());
    }
}

// ---------------------------------------------------------------------------
// Serializer + Deserializer: sentinel wrappers (Bind / Mutate / Negate).

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename = "@NexusBind")]
struct Bind(String);

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename = "@NexusMutate")]
struct Mutate<T>(T);

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename = "@NexusNegate")]
struct Negate<T>(T);

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Point {
    horizontal: f64,
    vertical: f64,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
enum Status {
    Active,
    Archived,
}

mod sentinel_bind {
    use super::*;

    #[test]
    fn bare_bind() {
        let b = Bind("horizontal".into());
        assert_eq!(to_string_nexus(&b).unwrap(), "@horizontal");
    }

    #[test]
    fn bind_roundtrip() {
        let b = Bind("counter".into());
        let s = to_string_nexus(&b).unwrap();
        let back: Bind = from_str_nexus(&s).unwrap();
        assert_eq!(back, b);
    }

    #[test]
    fn bind_kebab_case() {
        let b = Bind("customer-tier".into());
        assert_eq!(to_string_nexus(&b).unwrap(), "@customer-tier");
    }

    #[test]
    fn bind_rejects_uppercase_leader() {
        let b = Bind("Customer".into());
        assert!(to_string_nexus(&b).is_err());
    }

    #[test]
    fn bind_rejects_digit_leader() {
        let b = Bind("1name".into());
        assert!(to_string_nexus(&b).is_err());
    }
}

mod sentinel_mutate {
    use super::*;

    #[test]
    fn mutate_prefix_on_record() {
        let m = Mutate(Point { horizontal: 0.0, vertical: 0.0 });
        assert_eq!(to_string_nexus(&m).unwrap(), "~(Point 0.0 0.0)");
    }

    #[test]
    fn mutate_roundtrip() {
        let m = Mutate(Point { horizontal: 1.5, vertical: -2.5 });
        let s = to_string_nexus(&m).unwrap();
        let back: Mutate<Point> = from_str_nexus(&s).unwrap();
        assert_eq!(back, m);
    }
}

mod sentinel_negate {
    use super::*;

    #[test]
    fn negate_prefix_on_variant() {
        let n = Negate(Status::Active);
        assert_eq!(to_string_nexus(&n).unwrap(), "!Active");
    }

    #[test]
    fn negate_roundtrip() {
        let n = Negate(Status::Archived);
        let s = to_string_nexus(&n).unwrap();
        let back: Negate<Status> = from_str_nexus(&s).unwrap();
        assert_eq!(back, n);
    }
}

// ---------------------------------------------------------------------------
// Dialect isolation: sentinel wrappers must not work in nota mode.

mod dialect_isolation {
    use super::*;

    #[test]
    fn bind_rejected_in_nota_ser() {
        let b = Bind("x".into());
        let err = to_string(&b).unwrap_err();
        assert!(
            matches!(err, Error::Custom(ref msg) if msg.contains("nota dialect")),
            "unexpected error: {err:?}",
        );
    }

    #[test]
    fn bind_rejected_in_nota_de() {
        // Can't directly parse `@h` in nota — lexer rejects `@`.
        // Sentinel dispatch happens deeper; test the ser side which
        // rejects earlier.
        let result: Result<Bind, _> = from_str("@x");
        assert!(result.is_err());
    }

    #[test]
    fn plain_nota_types_work_in_nexus_mode() {
        // Nexus is a superset — plain records round-trip identically.
        let p = Point { horizontal: 3.0, vertical: 4.0 };
        let s = to_string_nexus(&p).unwrap();
        assert_eq!(s, "(Point 3.0 4.0)");
        let back: Point = from_str_nexus(&s).unwrap();
        assert_eq!(back, p);
    }
}

// ---------------------------------------------------------------------------
// Dialect accessor.

mod introspection {
    use super::*;

    #[test]
    fn lexer_dialect_accessor() {
        assert_eq!(Lexer::new("").dialect(), Dialect::Nota);
        assert_eq!(Lexer::nexus("").dialect(), Dialect::Nexus);
    }
}
