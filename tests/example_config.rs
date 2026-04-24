//! Phase 5 — parse a realistic `.nota` document through nota-serde.
//!
//! Covers: nested structs, enums with mixed variant kinds (unit +
//! newtype + struct), Vec of structs, Option, maps, multiline
//! strings, comments. Exercises the full public API on a single
//! document under the positional-records syntax.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Project {
    name: String,
    version: String,
    description: String,
    authors: Vec<String>,
    dependencies: Vec<Dep>,
    flags: BTreeMap<String, bool>,
    license: License,
    status: Status,
    release_notes: Option<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Dep {
    name: String,
    version: String,
    features: Vec<String>,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[allow(clippy::enum_variant_names)]
enum License {
    LicenseOfNonAuthority,
    Mit,
    Apache2,
    Dual { primary: String, fallback: String },
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
enum Status {
    Alpha,
    Beta,
    Released(String),
    Archived { reason: String, at_commit: String },
}

fn sample() -> Project {
    let mut flags = BTreeMap::new();
    flags.insert("debug".to_string(), true);
    flags.insert("strict".to_string(), false);
    flags.insert("verbose".to_string(), true);

    Project {
        name: "nota-serde".into(),
        version: "0.1.0".into(),
        description: "Rust serde implementation of the nota data format.\nData-layer only; nexus adds the messaging layer.".into(),
        authors: vec!["ligoldragon".into()],
        dependencies: vec![
            Dep { name: "serde".into(), version: "1".into(), features: vec!["derive".into()] },
            Dep { name: "thiserror".into(), version: "2".into(), features: vec![] },
        ],
        flags,
        license: License::LicenseOfNonAuthority,
        status: Status::Released("2026-04-23".into()),
        release_notes: None,
    }
}

#[test]
fn roundtrip_realistic_document() {
    let doc = sample();
    let text = nota_serde_core::to_string(&doc).expect("serialize");

    // Spot-check stable substrings. Fields are positional in
    // source-declaration order — name first. Ident-shaped strings
    // emit bare; strings starting with a digit (e.g. "0.1.0")
    // can't go bare and stay in `[ ]`.
    assert!(text.starts_with("(Project nota-serde [0.1.0]"));
    assert!(text.contains("LicenseOfNonAuthority"));
    // The version-string literal "2026-04-23" starts with a digit so
    // it can't go bare; stays in `[ ]`.
    assert!(text.contains("(Released [2026-04-23])"));
    // Option<T>::None renders as bare `None`; final position.
    assert!(text.ends_with("None)"));

    // Canonical map sort: debug < strict < verbose. Keys are
    // ident-shaped so they emit bare.
    let d = text.find("(debug ").unwrap();
    let s = text.find("(strict ").unwrap();
    let v = text.find("(verbose ").unwrap();
    assert!(d < s && s < v, "map entries not sorted: {text}");

    let back: Project = nota_serde_core::from_str(&text).expect("deserialize");
    assert_eq!(back, doc);
}

#[test]
fn roundtrip_with_archived_status_variant() {
    let mut doc = sample();
    doc.status = Status::Archived {
        reason: "superseded by nexus-serde".into(),
        at_commit: "abcdef".into(),
    };
    doc.release_notes = Some("see report 007".into());

    let text = nota_serde_core::to_string(&doc).expect("serialize");
    // Struct-variant in positional form: (Archived reason-val at_commit-val).
    // "superseded by nexus-serde" has a space → bracketed; "abcdef"
    // is ident-shaped → bare.
    assert!(text.contains("(Archived [superseded by nexus-serde] abcdef)"));
    // Option::Some(x) renders transparently; content has a space.
    assert!(text.contains("[see report 007]"));

    let back: Project = nota_serde_core::from_str(&text).expect("deserialize");
    assert_eq!(back, doc);
}

#[test]
fn parse_hand_written_document() {
    // What a developer would write — positional, indented, with
    // comments scattered across positions. Parser must tolerate.
    let text = r#"
        ;; Demo project manifest
        (Project
          [tiny]
          [0.0.1]
          [|
            two
            lines
          |]
          <[anon]>
          ;; no deps yet
          <>
          <([debug] true)>
          Mit
          Alpha
          None)
    "#;
    let p: Project = nota_serde_core::from_str(text).expect("parse hand-written");
    assert_eq!(p.name, "tiny");
    assert_eq!(p.version, "0.0.1");
    assert_eq!(p.description, "two\nlines");
    assert_eq!(p.authors, vec!["anon"]);
    assert_eq!(p.dependencies.len(), 0);
    assert_eq!(p.flags.get("debug"), Some(&true));
    assert!(matches!(p.license, License::Mit));
    assert!(matches!(p.status, Status::Alpha));
    assert_eq!(p.release_notes, None);
}
