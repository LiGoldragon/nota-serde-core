//! Nexus-dialect integration tests — lexer extensions and sentinel
//! dispatch on `Bind` / `Mutate` / `Negate` / `Validate` / `Subscribe`
//! / `AtomicBatch`.

use nota_serde_core::{
    from_str, from_str_nexus, to_string, to_string_nexus,
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
        assert_eq!(
            lex_nexus("~@!"),
            vec![Token::Tilde, Token::At, Token::Bang]
        );
    }

    #[test]
    fn question_token() {
        assert_eq!(lex_nexus("?"), vec![Token::Question]);
    }

    #[test]
    fn star_token() {
        assert_eq!(lex_nexus("*"), vec![Token::Star]);
    }

    #[test]
    fn five_sigils_in_sequence() {
        assert_eq!(
            lex_nexus("~ ! ? * @"),
            vec![
                Token::Tilde,
                Token::Bang,
                Token::Question,
                Token::Star,
                Token::At,
            ]
        );
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
    fn question_in_nota_mode_is_error() {
        let mut l = Lexer::new("?");
        assert!(l.next_token().is_err());
    }

    #[test]
    fn star_in_nota_mode_is_error() {
        let mut l = Lexer::new("*");
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
        assert_eq!(
            lex_nexus("{| |}"),
            vec![Token::LBracePipe, Token::RBracePipe]
        );
    }

    #[test]
    fn pattern_delim() {
        assert_eq!(
            lex_nexus("(| |)"),
            vec![Token::LParenPipe, Token::RParenPipe]
        );
    }

    #[test]
    fn pattern_delim_with_content() {
        let toks = lex_nexus("(| Point @h |)");
        assert_eq!(
            toks,
            vec![
                Token::LParenPipe,
                Token::Ident("Point".into()),
                Token::At,
                Token::Ident("h".into()),
                Token::RParenPipe,
            ]
        );
    }

    #[test]
    fn atomic_batch_delim_empty() {
        assert_eq!(
            lex_nexus("[||]"),
            vec![Token::LBracketPipe, Token::RBracketPipe]
        );
    }

    #[test]
    fn atomic_batch_delim_with_space() {
        assert_eq!(
            lex_nexus("[| |]"),
            vec![Token::LBracketPipe, Token::RBracketPipe]
        );
    }

    #[test]
    fn atomic_batch_delim_with_content() {
        let toks = lex_nexus("[| 1 2 3 |]");
        assert_eq!(
            toks,
            vec![
                Token::LBracketPipe,
                Token::Int(1),
                Token::Int(2),
                Token::Int(3),
                Token::RBracketPipe,
            ]
        );
    }

    #[test]
    fn pipe_followed_by_unknown_rejected() {
        // `|` followed by neither `)`, `}`, nor `]` is a lexer error.
        let mut l = Lexer::nexus("|x");
        assert!(l.next_token().is_err());
    }

    #[test]
    fn left_angle_in_nexus_rejected_as_reserved() {
        // `<` and `>` are reserved in both dialects (future
        // comparison operators).
        let mut l = Lexer::nexus("<");
        let err = l.next_token().unwrap_err().to_string();
        assert!(err.contains("reserved"), "got {err}");
    }

    #[test]
    fn right_angle_in_nexus_rejected_as_reserved() {
        let mut l = Lexer::nexus(">");
        let err = l.next_token().unwrap_err().to_string();
        assert!(err.contains("reserved"), "got {err}");
    }
}

// ---------------------------------------------------------------------------
// Serializer + Deserializer: sentinel wrappers (Bind / Mutate / Negate /
// Validate / Subscribe / AtomicBatch).

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
#[serde(rename = "@NexusValidate")]
struct Validate<T>(T);

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename = "@NexusSubscribe")]
struct Subscribe<T>(T);

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[serde(rename = "@NexusAtomicBatch")]
struct AtomicBatch<T>(Vec<T>);

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

mod sentinel_validate {
    use super::*;

    #[test]
    fn validate_prefix_on_int() {
        let v = Validate(5i32);
        assert_eq!(to_string_nexus(&v).unwrap(), "?5");
    }

    #[test]
    fn validate_prefix_on_record() {
        let v = Validate(Point { horizontal: 3.0, vertical: 4.0 });
        assert_eq!(to_string_nexus(&v).unwrap(), "?(Point 3.0 4.0)");
    }

    #[test]
    fn validate_roundtrip() {
        let v = Validate(Point { horizontal: 1.0, vertical: 2.0 });
        let s = to_string_nexus(&v).unwrap();
        let back: Validate<Point> = from_str_nexus(&s).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn validate_rejected_in_nota_ser() {
        let v = Validate(7i32);
        let err = to_string(&v).unwrap_err();
        assert!(
            matches!(err, Error::Custom(ref msg) if msg.contains("nota dialect")),
            "unexpected error: {err:?}"
        );
    }
}

mod sentinel_subscribe {
    use super::*;

    #[test]
    fn subscribe_prefix_on_record() {
        let v = Subscribe(Point { horizontal: 0.0, vertical: 0.0 });
        assert_eq!(to_string_nexus(&v).unwrap(), "*(Point 0.0 0.0)");
    }

    #[test]
    fn subscribe_roundtrip() {
        let v = Subscribe(Point { horizontal: 9.0, vertical: -1.0 });
        let s = to_string_nexus(&v).unwrap();
        let back: Subscribe<Point> = from_str_nexus(&s).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn subscribe_rejected_in_nota_ser() {
        let v = Subscribe(7i32);
        assert!(to_string(&v).is_err());
    }
}

mod sentinel_atomic_batch {
    use super::*;

    #[test]
    fn empty_atomic_batch() {
        let b: AtomicBatch<i32> = AtomicBatch(vec![]);
        assert_eq!(to_string_nexus(&b).unwrap(), "[||]");
    }

    #[test]
    fn atomic_batch_of_ints() {
        let b = AtomicBatch(vec![1, 2, 3]);
        assert_eq!(to_string_nexus(&b).unwrap(), "[| 1 2 3 |]");
    }

    #[test]
    fn atomic_batch_of_records() {
        let b = AtomicBatch(vec![
            Point { horizontal: 1.0, vertical: 2.0 },
            Point { horizontal: 3.0, vertical: 4.0 },
        ]);
        assert_eq!(
            to_string_nexus(&b).unwrap(),
            "[| (Point 1.0 2.0) (Point 3.0 4.0) |]"
        );
    }

    #[test]
    fn atomic_batch_roundtrip_ints() {
        let b = AtomicBatch(vec![10, 20, 30]);
        let s = to_string_nexus(&b).unwrap();
        let back: AtomicBatch<i32> = from_str_nexus(&s).unwrap();
        assert_eq!(back, b);
    }

    #[test]
    fn atomic_batch_roundtrip_empty() {
        let b: AtomicBatch<i32> = AtomicBatch(vec![]);
        let s = to_string_nexus(&b).unwrap();
        let back: AtomicBatch<i32> = from_str_nexus(&s).unwrap();
        assert_eq!(back, b);
    }

    #[test]
    fn atomic_batch_roundtrip_records() {
        let b = AtomicBatch(vec![
            Point { horizontal: 0.5, vertical: 0.25 },
            Point { horizontal: -1.0, vertical: 1.5 },
        ]);
        let s = to_string_nexus(&b).unwrap();
        let back: AtomicBatch<Point> = from_str_nexus(&s).unwrap();
        assert_eq!(back, b);
    }

    #[test]
    fn atomic_batch_rejected_in_nota_ser() {
        let b = AtomicBatch(vec![1i32]);
        assert!(to_string(&b).is_err());
    }
}

mod nested_wrappers {
    use super::*;

    #[test]
    fn validate_wrapping_mutate() {
        let v = Validate(Mutate(7i32));
        assert_eq!(to_string_nexus(&v).unwrap(), "?~7");
        let back: Validate<Mutate<i32>> = from_str_nexus("?~7").unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn validate_wrapping_record() {
        let v = Validate(Mutate(Point { horizontal: 1.0, vertical: 2.0 }));
        let text = to_string_nexus(&v).unwrap();
        assert_eq!(text, "?~(Point 1.0 2.0)");
        let back: Validate<Mutate<Point>> = from_str_nexus(&text).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn subscribe_wrapping_negate() {
        let v = Subscribe(Negate(Status::Active));
        assert_eq!(to_string_nexus(&v).unwrap(), "*!Active");
        let back: Subscribe<Negate<Status>> = from_str_nexus("*!Active").unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn atomic_batch_of_mutates() {
        let b = AtomicBatch(vec![
            Mutate(Point { horizontal: 1.0, vertical: 2.0 }),
            Mutate(Point { horizontal: 3.0, vertical: 4.0 }),
        ]);
        let text = to_string_nexus(&b).unwrap();
        assert_eq!(text, "[| ~(Point 1.0 2.0) ~(Point 3.0 4.0) |]");
        let back: AtomicBatch<Mutate<Point>> = from_str_nexus(&text).unwrap();
        assert_eq!(back, b);
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
        let result: Result<Bind, _> = from_str("@x");
        assert!(result.is_err());
    }

    #[test]
    fn validate_rejected_in_nota_de() {
        let result: Result<Validate<i32>, _> = from_str("?5");
        assert!(result.is_err());
    }

    #[test]
    fn subscribe_rejected_in_nota_de() {
        let result: Result<Subscribe<Status>, _> = from_str("*Active");
        assert!(result.is_err());
    }

    #[test]
    fn atomic_batch_rejected_in_nota_de() {
        let result: Result<AtomicBatch<i32>, _> = from_str("[| 1 2 |]");
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
