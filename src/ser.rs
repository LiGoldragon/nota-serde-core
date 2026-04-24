//! Serializer emitting canonical nota or nexus text.
//!
//! Records are positional: `(TypeName v1 v2 …)` with fields in
//! source-declaration order. Newtype structs wrap: `struct Id(u32)` →
//! `(Id 42)`. Multi-field unnamed structs (tuple structs with len ≥ 2)
//! are forbidden — use a named-field struct instead. Maps sort by
//! serialized key bytes. Floats always contain `.`. Strings are
//! `[ inline ]` or `[| multiline |]`. Bytes are `#<lowercase-hex>`.
//!
//! [`Dialect`] selects the grammar. In [`Dialect::Nexus`] the
//! serializer additionally dispatches on sentinel newtype-struct names
//! to emit nexus sigils ([`BIND_SENTINEL`] → `@name`,
//! [`MUTATE_SENTINEL`] → `~value`, [`NEGATE_SENTINEL`] → `!value`).

use std::fmt::Write as _;

use serde::{ser, Serialize};

use crate::error::{Error, Result};
use crate::lexer::Dialect;

/// Newtype-struct name that dispatches as a nexus bind (`@name`).
/// Consumers derive `#[serde(rename = "@NexusBind")]` on the wrapper
/// type to opt in.
pub const BIND_SENTINEL: &str = "@NexusBind";
/// Newtype-struct name dispatching as a nexus mutation marker (`~value`).
pub const MUTATE_SENTINEL: &str = "@NexusMutate";
/// Newtype-struct name dispatching as a nexus negation marker (`!value`).
pub const NEGATE_SENTINEL: &str = "@NexusNegate";

pub fn to_string<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    to_string_with(value, Dialect::Nota)
}

pub fn to_string_nexus<T: Serialize + ?Sized>(value: &T) -> Result<String> {
    to_string_with(value, Dialect::Nexus)
}

pub fn to_string_with<T: Serialize + ?Sized>(value: &T, dialect: Dialect) -> Result<String> {
    let mut ser = Serializer::with_dialect(dialect);
    value.serialize(&mut ser)?;
    Ok(ser.output)
}

pub struct Serializer {
    output: String,
    dialect: Dialect,
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

/// Bind names must follow the camelCase or kebab-case identifier
/// classes: first char lowercase or `_`, body in `[a-z0-9_-]`. No
/// uppercase (reserved for PascalCase types) and no leading digit
/// or `-`.
fn is_valid_bind_name(s: &str) -> bool {
    let mut chars = s.chars();
    let first_ok = matches!(
        chars.next(),
        Some(c) if c.is_ascii_lowercase() || c == '_'
    );
    let rest_ok = chars.all(
        |c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_'
    );
    first_ok && rest_ok
}

impl Serializer {
    pub fn new() -> Self {
        Self::with_dialect(Dialect::Nota)
    }

    pub fn with_dialect(dialect: Dialect) -> Self {
        Self { output: String::new(), dialect }
    }

    pub fn into_string(self) -> String {
        self.output
    }

    pub fn dialect(&self) -> Dialect {
        self.dialect
    }

    fn append(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn write_str_literal(&mut self, v: &str) -> Result<()> {
        if v.contains("|]") {
            return Err(Error::StringContainsMultilineCloser);
        }
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
        if s.contains('.') || s.contains('e') || s.contains('E') {
            self.output.push_str(s);
        } else {
            self.output.push_str(s);
            self.output.push_str(".0");
        }
    }

    fn serialize_bind<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        let mut sub = Serializer::with_dialect(self.dialect);
        value.serialize(&mut sub)?;
        let s = sub.output;
        // Inner may come through bare (canonical for ident-shaped
        // strings) or `[name]` (bracketed form). Strip the brackets
        // if present so validation sees just the content.
        let inner = if s.starts_with('[') && s.ends_with(']') && s.len() >= 2 {
            &s[1..s.len() - 1]
        } else {
            s.as_str()
        };
        if !is_valid_bind_name(inner) {
            return Err(Error::Custom(format!(
                "Bind name must be camelCase or kebab-case (first char `[a-z_]`, body `[a-z0-9_-]`), got {inner:?}"
            )));
        }
        self.output.push('@');
        self.output.push_str(inner);
        Ok(())
    }

    fn reject_sentinel_in_nota(&self, name: &str) -> Result<()> {
        Err(Error::Custom(format!(
            "sentinel newtype-struct `{name}` is not valid in nota dialect; serialize via nexus (`to_string_nexus`) instead"
        )))
    }
}

impl Default for Serializer {
    fn default() -> Self { Self::new() }
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
        if matches!(name, BIND_SENTINEL | MUTATE_SENTINEL | NEGATE_SENTINEL) {
            if self.dialect != Dialect::Nexus {
                return self.reject_sentinel_in_nota(name);
            }
            return match name {
                BIND_SENTINEL => self.serialize_bind(value),
                MUTATE_SENTINEL => {
                    self.output.push('~');
                    value.serialize(self)
                }
                NEGATE_SENTINEL => {
                    self.output.push('!');
                    value.serialize(self)
                }
                _ => unreachable!(),
            };
        }
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
        Err(Error::MultiFieldTupleStructForbidden { name: variant, len })
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
        // Positional: field names live in the schema, not the text.
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
        let mut sub = Serializer::with_dialect(self.ser.dialect);
        key.serialize(&mut sub)?;
        self.current_key = Some(sub.output);
        Ok(())
    }

    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<()> {
        let mut sub = Serializer::with_dialect(self.ser.dialect);
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
