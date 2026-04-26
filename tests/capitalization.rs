//! Capitalization rules at parse time.
//!
//! Type and variant names — the head of a record `(Foo …)`, the
//! token of a unit struct `Foo`, an enum variant `Active` or the
//! head of a payload variant `(Sized w h)` — must be PascalCase
//! (first char ASCII uppercase). The deserializer rejects head
//! identifiers that fail this rule before attempting to match
//! against the Rust schema name.
//!
//! Bare-identifier strings in *value* position remain
//! unrestricted: PascalCase / camelCase / kebab-case all serve as
//! `String` literals when the schema expects one.

use nota_serde_core::{from_str, is_pascal_case};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Helper itself.

mod helper {
    use super::is_pascal_case;

    #[test]
    fn pascal_case_starts_uppercase() {
        assert!(is_pascal_case("Foo"));
        assert!(is_pascal_case("FOO"));
        assert!(is_pascal_case("F"));
        assert!(is_pascal_case("FooBar"));
    }

    #[test]
    fn camel_case_rejected() {
        assert!(!is_pascal_case("foo"));
        assert!(!is_pascal_case("fooBar"));
        assert!(!is_pascal_case("f"));
    }

    #[test]
    fn kebab_case_rejected() {
        assert!(!is_pascal_case("foo-bar"));
    }

    #[test]
    fn underscore_leader_rejected() {
        // `_` is camelCase-kindred per the nota spec.
        assert!(!is_pascal_case("_Foo"));
        assert!(!is_pascal_case("_foo"));
    }

    #[test]
    fn empty_rejected() {
        assert!(!is_pascal_case(""));
    }
}

// ---------------------------------------------------------------------------
// Head-position enforcement: struct, unit struct, newtype, variant.

mod head_must_be_pascal {
    use super::*;

    #[derive(Deserialize, Debug, PartialEq)]
    struct Point { horizontal: f64, vertical: f64 }

    #[derive(Deserialize, Debug, PartialEq)]
    struct Stop;

    #[derive(Deserialize, Debug, PartialEq)]
    struct Slot(u64);

    #[derive(Deserialize, Debug, PartialEq)]
    enum Status {
        Active,
        Sized { w: f64, h: f64 },
    }

    #[test]
    fn lowercase_struct_head_rejected() {
        let err = from_str::<Point>("(point 1.0 2.0)").unwrap_err().to_string();
        assert!(
            err.contains("PascalCase") && err.contains("point"),
            "expected PascalCase rule message, got: {err}"
        );
    }

    #[test]
    fn lowercase_unit_struct_rejected() {
        let err = from_str::<Stop>("stop").unwrap_err().to_string();
        assert!(
            err.contains("PascalCase") && err.contains("stop"),
            "expected PascalCase rule message, got: {err}"
        );
    }

    #[test]
    fn lowercase_newtype_head_rejected() {
        let err = from_str::<Slot>("(slot 100)").unwrap_err().to_string();
        assert!(
            err.contains("PascalCase") && err.contains("slot"),
            "expected PascalCase rule message, got: {err}"
        );
    }

    #[test]
    fn lowercase_unit_variant_rejected() {
        let err = from_str::<Status>("active").unwrap_err().to_string();
        assert!(
            err.contains("PascalCase") && err.contains("active"),
            "expected PascalCase rule message, got: {err}"
        );
    }

    #[test]
    fn lowercase_payload_variant_rejected() {
        let err = from_str::<Status>("(sized 3.0 4.0)").unwrap_err().to_string();
        assert!(
            err.contains("PascalCase") && err.contains("sized"),
            "expected PascalCase rule message, got: {err}"
        );
    }

    #[test]
    fn pascal_head_works() {
        let p: Point = from_str("(Point 1.0 2.0)").unwrap();
        assert_eq!(p, Point { horizontal: 1.0, vertical: 2.0 });

        let _: Stop = from_str("Stop").unwrap();
        let s: Slot = from_str("(Slot 100)").unwrap();
        assert_eq!(s, Slot(100));

        let v: Status = from_str("Active").unwrap();
        assert_eq!(v, Status::Active);

        let v: Status = from_str("(Sized 3.0 4.0)").unwrap();
        assert_eq!(v, Status::Sized { w: 3.0, h: 4.0 });
    }

    #[test]
    fn pascal_head_wrong_name_gives_specific_error() {
        // PascalCase passes the rule; the schema-mismatch error is
        // distinct from the rule-violation error.
        let err = from_str::<Point>("(Line 1.0 2.0)").unwrap_err().to_string();
        assert!(
            err.contains("expected struct `Point`") && err.contains("Line"),
            "expected schema-mismatch message, got: {err}"
        );
        // Importantly, the PascalCase-rule wording must not appear here.
        assert!(
            !err.contains("must be PascalCase"),
            "schema-mismatch path leaked the rule wording: {err}"
        );
    }

    #[test]
    fn underscore_leading_struct_head_rejected() {
        // `_Foo` is camelCase-kindred per the nota spec; reject in head
        // position.
        let err = from_str::<Point>("(_Point 1.0 2.0)").unwrap_err().to_string();
        assert!(
            err.contains("PascalCase"),
            "expected PascalCase rule message for `_Point`, got: {err}"
        );
    }
}

// ---------------------------------------------------------------------------
// Value-position bare-strings keep working — Idea A applies to head
// position only. PascalCase / camelCase / kebab-case bare-string
// literals continue to round-trip per the prior user clarification.

mod value_position_bare_strings_unchanged {
    use super::*;

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct Tag(String);

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct Person { name: String, role: String }

    #[test]
    fn pascal_bare_string_in_newtype_value_works() {
        // `Tag` is PascalCase head; `User` is PascalCase string value.
        let t: Tag = from_str("(Tag User)").unwrap();
        assert_eq!(t, Tag("User".into()));
    }

    #[test]
    fn pascal_bare_string_in_struct_field_works() {
        let p: Person = from_str("(Person User Admin)").unwrap();
        assert_eq!(p, Person { name: "User".into(), role: "Admin".into() });
    }

    #[test]
    fn mixed_case_bare_strings_in_seq_work() {
        let v: Vec<String> = from_str("[User myField some-tag]").unwrap();
        assert_eq!(v, vec!["User", "myField", "some-tag"]);
    }
}
