//! Serializer emitting canonical nota text.
//!
//! Records are positional: `(TypeName v1 v2 …)` with fields in
//! source-declaration order. Newtype structs wrap: `struct Id(u32)` →
//! `(Id 42)`. Multi-field unnamed structs (tuple structs with len ≥ 2)
//! are forbidden — use a named-field struct instead. Maps sort by
//! serialized key bytes. Floats always contain `.`. Strings are
//! `[ inline ]` or `[| multiline |]`. Bytes are `#<lowercase-hex>`.

use std::fmt::Write as _;

use serde::{ser, Serialize};

use crate::error::{Error, Result};

pub fn to_string<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    let mut ser = Serializer { output: String::new() };
    value.serialize(&mut ser)?;
    Ok(ser.output)
}

pub struct Serializer {
    output: String,
}

/// A string value may be emitted bare (without `[ ]`) when its
/// content is a non-empty ident-class token (PascalCase / camelCase
/// / kebab-case) that is not one of the reserved keywords (`true`,
/// `false`, `None`). This matches what [`crate::lexer`] accepts as
/// `Token::Ident`.
fn is_bare_string_eligible(v: &str) -> bool {
    if v.is_empty() {
        return false;
    }
    if matches!(v, "true" | "false" | "None") {
        return false;
    }
    let mut chars = v.chars();
    let first = chars.next().expect("checked non-empty above");
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

impl Serializer {
    fn append(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn write_str_literal(&mut self, v: &str) -> Result<()> {
        if v.contains("|]") {
            return Err(Error::StringContainsMultilineCloser);
        }
        // Bare-identifier form: if the content is a valid ident-class
        // token and not a reserved keyword, emit without delimiters.
        // Canonical form favours bare for readability of
        // identifier-shaped string values (e.g. `nota-serde`).
        if is_bare_string_eligible(v) {
            self.output.push_str(v);
            return Ok(());
        }
        let needs_multiline = v.contains(']') || v.contains('\n');
        if needs_multiline {
            self.output.push_str("[|\n");
            self.output.push_str(v);
            if !v.ends_with('\n') {
                self.output.push('\n');
            }
            self.output.push_str("|]");
        } else {
            self.output.push('[');
            self.output.push_str(v);
            self.output.push(']');
        }
        Ok(())
    }

    fn write_float(&mut self, s: &str) {
        // s is the default shortest-roundtrip repr. Ensure it contains `.`
        // (or an exponent) so it's unambiguously a float in nota.
        if s.contains('.') || s.contains('e') || s.contains('E') {
            self.output.push_str(s);
        } else {
            self.output.push_str(s);
            self.output.push_str(".0");
        }
    }
}

impl<'a> ser::Serializer for &'a mut Serializer {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = SeqSerializer<'a>;
    type SerializeTuple = SeqSerializer<'a>;
    type SerializeTupleStruct = NamedSeqSerializer<'a>;
    type SerializeTupleVariant = NamedSeqSerializer<'a>;
    type SerializeMap = MapSerializer<'a>;
    type SerializeStruct = StructSerializer<'a>;
    type SerializeStructVariant = StructSerializer<'a>;

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.append(if v { "true" } else { "false" });
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<()> { write!(self.output, "{v}").unwrap(); Ok(()) }
    fn serialize_i16(self, v: i16) -> Result<()> { write!(self.output, "{v}").unwrap(); Ok(()) }
    fn serialize_i32(self, v: i32) -> Result<()> { write!(self.output, "{v}").unwrap(); Ok(()) }
    fn serialize_i64(self, v: i64) -> Result<()> { write!(self.output, "{v}").unwrap(); Ok(()) }
    fn serialize_i128(self, v: i128) -> Result<()> { write!(self.output, "{v}").unwrap(); Ok(()) }
    fn serialize_u8(self, v: u8) -> Result<()> { write!(self.output, "{v}").unwrap(); Ok(()) }
    fn serialize_u16(self, v: u16) -> Result<()> { write!(self.output, "{v}").unwrap(); Ok(()) }
    fn serialize_u32(self, v: u32) -> Result<()> { write!(self.output, "{v}").unwrap(); Ok(()) }
    fn serialize_u64(self, v: u64) -> Result<()> { write!(self.output, "{v}").unwrap(); Ok(()) }
    fn serialize_u128(self, v: u128) -> Result<()> { write!(self.output, "{v}").unwrap(); Ok(()) }

    fn serialize_f32(self, v: f32) -> Result<()> {
        if !v.is_finite() { return Err(Error::NonFiniteFloat); }
        let s = format!("{v}");
        self.write_float(&s);
        Ok(())
    }

    fn serialize_f64(self, v: f64) -> Result<()> {
        if !v.is_finite() { return Err(Error::NonFiniteFloat); }
        let s = format!("{v}");
        self.write_float(&s);
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<()> {
        let mut buf = [0u8; 4];
        self.write_str_literal(v.encode_utf8(&mut buf))
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        self.write_str_literal(v)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.output.push('#');
        for b in v {
            write!(self.output, "{b:02x}").unwrap();
        }
        Ok(())
    }

    fn serialize_none(self) -> Result<()> {
        self.append("None");
        Ok(())
    }

    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<()> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        Err(Error::UnitForbidden)
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<()> {
        self.append(name);
        Ok(())
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<()> {
        self.append(variant);
        Ok(())
    }

    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<()> {
        self.output.push('(');
        self.output.push_str(name);
        self.output.push(' ');
        value.serialize(&mut *self)?;
        self.output.push(')');
        Ok(())
    }

    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<()> {
        self.output.push('(');
        self.output.push_str(variant);
        self.output.push(' ');
        value.serialize(&mut *self)?;
        self.output.push(')');
        Ok(())
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq> {
        self.output.push('<');
        Ok(SeqSerializer { ser: self, first: true })
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        self.output.push('<');
        Ok(SeqSerializer { ser: self, first: true })
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        // Multi-field tuple structs have no schema field names — nota
        // can't map position → meaning. Single-field tuple structs go
        // through serialize_newtype_struct, not here. Reject at any len.
        Err(Error::MultiFieldTupleStructForbidden { name, len })
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        // Multi-field tuple variants (e.g. `Pair(i32, i32)`) have no
        // schema field names. Single-field variants go through
        // serialize_newtype_variant instead. Reject here.
        Err(Error::MultiFieldTupleStructForbidden {
            name: variant,
            len,
        })
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap> {
        Ok(MapSerializer {
            ser: self,
            entries: Vec::new(),
            current_key: None,
        })
    }

    fn serialize_struct(
        self,
        name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct> {
        self.output.push('(');
        self.output.push_str(name);
        Ok(StructSerializer { ser: self })
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.output.push('(');
        self.output.push_str(variant);
        Ok(StructSerializer { ser: self })
    }
}

// ---------- sub-serializers ----------

pub struct SeqSerializer<'a> {
    ser: &'a mut Serializer,
    first: bool,
}

impl<'a> SeqSerializer<'a> {
    fn element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        if !self.first {
            self.ser.output.push(' ');
        }
        self.first = false;
        value.serialize(&mut *self.ser)
    }

    fn close(self) {
        self.ser.output.push('>');
    }
}

impl<'a> ser::SerializeSeq for SeqSerializer<'a> {
    type Ok = ();
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<()> { self.element(v) }
    fn end(self) -> Result<()> { self.close(); Ok(()) }
}

impl<'a> ser::SerializeTuple for SeqSerializer<'a> {
    type Ok = ();
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<()> { self.element(v) }
    fn end(self) -> Result<()> { self.close(); Ok(()) }
}

pub struct NamedSeqSerializer<'a> {
    ser: &'a mut Serializer,
}

impl<'a> NamedSeqSerializer<'a> {
    fn field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        self.ser.output.push(' ');
        value.serialize(&mut *self.ser)
    }
    fn close(self) {
        self.ser.output.push(')');
    }
}

impl<'a> ser::SerializeTupleStruct for NamedSeqSerializer<'a> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<()> { self.field(v) }
    fn end(self) -> Result<()> { self.close(); Ok(()) }
}

impl<'a> ser::SerializeTupleVariant for NamedSeqSerializer<'a> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, v: &T) -> Result<()> { self.field(v) }
    fn end(self) -> Result<()> { self.close(); Ok(()) }
}

pub struct StructSerializer<'a> {
    ser: &'a mut Serializer,
}

impl<'a> StructSerializer<'a> {
    fn field<T: Serialize + ?Sized>(&mut self, _key: &'static str, value: &T) -> Result<()> {
        // Positional: field names come from the Rust schema, not from
        // the text. Emit a single space before each value; the struct's
        // opening `(TypeName` was already written by serialize_struct.
        self.ser.output.push(' ');
        value.serialize(&mut *self.ser)
    }
    fn close(self) {
        self.ser.output.push(')');
    }
}

impl<'a> ser::SerializeStruct for StructSerializer<'a> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, v: &T) -> Result<()> {
        self.field(key, v)
    }
    fn end(self) -> Result<()> { self.close(); Ok(()) }
}

impl<'a> ser::SerializeStructVariant for StructSerializer<'a> {
    type Ok = ();
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, key: &'static str, v: &T) -> Result<()> {
        self.field(key, v)
    }
    fn end(self) -> Result<()> { self.close(); Ok(()) }
}

pub struct MapSerializer<'a> {
    ser: &'a mut Serializer,
    entries: Vec<(String, String)>,
    current_key: Option<String>,
}

impl<'a> ser::SerializeMap for MapSerializer<'a> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<()> {
        let mut sub = Serializer { output: String::new() };
        key.serialize(&mut sub)?;
        self.current_key = Some(sub.output);
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        let mut sub = Serializer { output: String::new() };
        value.serialize(&mut sub)?;
        let key = self.current_key.take().ok_or(Error::MapValueWithoutKey)?;
        self.entries.push((key, sub.output));
        Ok(())
    }

    fn end(mut self) -> Result<()> {
        self.entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        self.ser.output.push('<');
        let mut first = true;
        for (k, v) in &self.entries {
            if !first { self.ser.output.push(' '); }
            first = false;
            self.ser.output.push('(');
            self.ser.output.push_str(k);
            self.ser.output.push(' ');
            self.ser.output.push_str(v);
            self.ser.output.push(')');
        }
        self.ser.output.push('>');
        Ok(())
    }
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::to_string;
    use serde::Serialize;
    use std::collections::BTreeMap;

    #[test]
    fn primitives() {
        assert_eq!(to_string(&true).unwrap(), "true");
        assert_eq!(to_string(&false).unwrap(), "false");
        assert_eq!(to_string(&42i32).unwrap(), "42");
        assert_eq!(to_string(&-7i64).unwrap(), "-7");
        assert_eq!(to_string(&0u32).unwrap(), "0");
        assert_eq!(to_string(&2.5f64).unwrap(), "2.5");
        assert_eq!(to_string(&1.0f64).unwrap(), "1.0");
        assert_eq!(to_string(&-0.5f32).unwrap(), "-0.5");
        // Ident-shaped strings emit bare; content requiring brackets
        // goes through a separate test below.
        assert_eq!(to_string("hello").unwrap(), "hello");
        assert_eq!(to_string("kebab-name").unwrap(), "kebab-name");
        // Reserved keywords must stay bracketed so they round-trip
        // outside a bool / Option context.
        assert_eq!(to_string("true").unwrap(), "[true]");
        assert_eq!(to_string("None").unwrap(), "[None]");
        // Content with a space can't be bare.
        assert_eq!(to_string("hello world").unwrap(), "[hello world]");
    }

    #[test]
    fn string_needing_multiline() {
        assert_eq!(to_string("a]b").unwrap(), "[|\na]b\n|]");
        assert_eq!(to_string("line one\nline two").unwrap(), "[|\nline one\nline two\n|]");
    }

    #[test]
    fn string_with_multiline_closer_fails() {
        assert!(to_string("foo|]bar").is_err());
    }

    #[test]
    fn bytes_direct() {
        use serde::Serializer as _;
        let mut s = super::Serializer { output: String::new() };
        (&mut s).serialize_bytes(&[0xa1, 0xb2, 0xc3]).unwrap();
        assert_eq!(s.output, "#a1b2c3");
    }

    #[test]
    fn option() {
        let none: Option<i32> = None;
        assert_eq!(to_string(&none).unwrap(), "None");
        let some: Option<i32> = Some(7);
        assert_eq!(to_string(&some).unwrap(), "7");
    }

    #[test]
    fn unit_forbidden() {
        assert!(to_string(&()).is_err());
    }

    #[test]
    fn unit_struct() {
        #[derive(Serialize)]
        struct Marker;
        assert_eq!(to_string(&Marker).unwrap(), "Marker");
    }

    #[test]
    fn unit_variant() {
        #[derive(Serialize)]
        enum Status { Active, Archived }
        assert_eq!(to_string(&Status::Active).unwrap(), "Active");
        assert_eq!(to_string(&Status::Archived).unwrap(), "Archived");
    }

    #[test]
    fn newtype_struct_wraps() {
        #[derive(Serialize)]
        struct Id(u32);
        assert_eq!(to_string(&Id(42)).unwrap(), "(Id 42)");
    }

    #[test]
    fn newtype_variant_wrapped() {
        #[derive(Serialize)]
        enum E { V(i32) }
        assert_eq!(to_string(&E::V(7)).unwrap(), "(V 7)");
    }

    #[test]
    fn tuple_struct_rejected() {
        #[derive(Serialize)]
        struct Pair(i32, i32);
        let err = to_string(&Pair(3, 4)).unwrap_err();
        assert!(
            format!("{err}").contains("multi-field unnamed struct"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn tuple_variant_rejected() {
        #[derive(Serialize)]
        enum E { Pair(i32, i32) }
        let err = to_string(&E::Pair(3, 4)).unwrap_err();
        assert!(
            format!("{err}").contains("multi-field unnamed struct"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn seq() {
        let v = vec![1, 2, 3];
        assert_eq!(to_string(&v).unwrap(), "<1 2 3>");
    }

    #[test]
    fn tuple() {
        let t = (1i32, "a", true);
        assert_eq!(to_string(&t).unwrap(), "<1 a true>");
    }

    #[test]
    fn struct_() {
        #[derive(Serialize)]
        struct Point { horizontal: f64, vertical: f64 }
        assert_eq!(
            to_string(&Point { horizontal: 3.0, vertical: 4.0 }).unwrap(),
            "(Point 3.0 4.0)"
        );
    }

    #[test]
    fn struct_variant() {
        #[derive(Serialize)]
        enum Shape {
            Circle { radius: f64 },
        }
        assert_eq!(to_string(&Shape::Circle { radius: 2.0 }).unwrap(), "(Circle 2.0)");
    }

    #[test]
    fn nested_struct() {
        #[derive(Serialize)]
        struct Point { x: f64, y: f64 }
        #[derive(Serialize)]
        struct Line { start: Point, end: Point }
        let l = Line { start: Point { x: 0.0, y: 0.0 }, end: Point { x: 1.0, y: 2.0 } };
        assert_eq!(to_string(&l).unwrap(), "(Line (Point 0.0 0.0) (Point 1.0 2.0))");
    }

    #[test]
    fn map_canonical_sort() {
        let mut m = BTreeMap::new();
        m.insert("beta", 2);
        m.insert("alpha", 1);
        // Ident-shaped keys emit bare; the canonical sort compares
        // the serialised key bytes, so `alpha` < `beta`.
        assert_eq!(to_string(&m).unwrap(), "<(alpha 1) (beta 2)>");
    }

    #[test]
    fn map_canonical_sort_with_hashmap() {
        use std::collections::HashMap;
        let mut m: HashMap<&str, i32> = HashMap::new();
        m.insert("zeta", 26);
        m.insert("alpha", 1);
        m.insert("mu", 12);
        // HashMap iteration order is non-deterministic; canonical form
        // must still emit in sorted-by-serialised-key order.
        assert_eq!(
            to_string(&m).unwrap(),
            "<(alpha 1) (mu 12) (zeta 26)>"
        );
    }

    #[test]
    fn vec_of_structs() {
        #[derive(Serialize)]
        struct Point { x: i32, y: i32 }
        let pts = vec![Point { x: 0, y: 0 }, Point { x: 1, y: 1 }];
        assert_eq!(
            to_string(&pts).unwrap(),
            "<(Point 0 0) (Point 1 1)>"
        );
    }

    #[test]
    fn option_of_newtype_wraps() {
        #[derive(Serialize)]
        struct Id(u32);
        let some: Option<Id> = Some(Id(42));
        assert_eq!(to_string(&some).unwrap(), "(Id 42)");
    }

    #[test]
    fn empty_seq_and_struct() {
        let v: Vec<i32> = vec![];
        assert_eq!(to_string(&v).unwrap(), "<>");
        #[derive(Serialize)]
        struct Empty;
        assert_eq!(to_string(&Empty).unwrap(), "Empty");
    }
}
