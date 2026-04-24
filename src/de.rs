//! Deserializer: parse nota text into types implementing [`serde::Deserialize`].
//!
//! Token-stream-driven recursive descent. The lexer produces tokens; the
//! Deserializer peeks and consumes them driven by the visitor's demands.

use serde::de::{
    self, DeserializeSeed, EnumAccess, IntoDeserializer, MapAccess, SeqAccess,
    VariantAccess, Visitor,
};

use crate::error::{Error, Result};
use crate::lexer::{Lexer, Token};

pub fn from_str<'a, T: de::Deserialize<'a>>(input: &'a str) -> Result<T> {
    let mut de = Deserializer::new(input);
    let value = T::deserialize(&mut de)?;
    de.expect_end()?;
    Ok(value)
}

pub struct Deserializer<'de> {
    stream: TokenStream<'de>,
}

struct TokenStream<'a> {
    lexer: Lexer<'a>,
    peeked: Option<Token>,
}

impl<'a> TokenStream<'a> {
    fn new(input: &'a str) -> Self {
        Self { lexer: Lexer::new(input), peeked: None }
    }

    fn peek(&mut self) -> Result<Option<&Token>> {
        if self.peeked.is_none() {
            self.peeked = self.lexer.next_token()?;
        }
        Ok(self.peeked.as_ref())
    }

    fn next(&mut self) -> Result<Option<Token>> {
        if let Some(t) = self.peeked.take() {
            Ok(Some(t))
        } else {
            self.lexer.next_token()
        }
    }

    fn expect_next(&mut self) -> Result<Token> {
        self.next()?.ok_or_else(|| Error::Custom("unexpected end of input".into()))
    }

    fn expect_matching(&mut self, expected: &Token) -> Result<()> {
        let got = self.expect_next()?;
        if &got == expected {
            Ok(())
        } else {
            Err(Error::Custom(format!("expected {expected:?}, got {got:?}")))
        }
    }
}

impl<'de> Deserializer<'de> {
    fn new(input: &'de str) -> Self {
        Self { stream: TokenStream::new(input) }
    }

    fn expect_end(&mut self) -> Result<()> {
        match self.stream.next()? {
            None => Ok(()),
            Some(t) => Err(Error::Custom(format!("extra input after value: {t:?}"))),
        }
    }
}

fn int_to_i128(t: Token) -> Result<i128> {
    match t {
        Token::Int(i) => Ok(i),
        Token::UInt(u) => Err(Error::Custom(format!(
            "integer {u} exceeds i128::MAX; use a u128 field to hold it"
        ))),
        other => Err(Error::Custom(format!("expected integer, got {other:?}"))),
    }
}

fn int_to_u128(t: Token) -> Result<u128> {
    match t {
        Token::Int(i) if i >= 0 => Ok(i as u128),
        Token::Int(i) => Err(Error::Custom(format!(
            "negative integer {i} cannot be represented as unsigned"
        ))),
        Token::UInt(u) => Ok(u),
        other => Err(Error::Custom(format!("expected integer, got {other:?}"))),
    }
}

fn float(t: Token) -> Result<f64> {
    match t {
        Token::Float(f) => Ok(f),
        Token::Int(i) => Ok(i as f64),
        Token::UInt(u) => Ok(u as f64),
        other => Err(Error::Custom(format!("expected float, got {other:?}"))),
    }
}

impl<'de> de::Deserializer<'de> for &mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        Err(Error::Custom(
            "nota-serde does not support `deserialize_any` — types must be self-describing via their Deserialize impl".into(),
        ))
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.stream.expect_next()? {
            Token::Bool(b) => visitor.visit_bool(b),
            other => Err(Error::Custom(format!("expected bool, got {other:?}"))),
        }
    }

    fn deserialize_i8<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let i = int_to_i128(self.stream.expect_next()?)?;
        v.visit_i8(i.try_into().map_err(|_| Error::Custom(format!("{i} out of i8 range")))?)
    }
    fn deserialize_i16<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let i = int_to_i128(self.stream.expect_next()?)?;
        v.visit_i16(i.try_into().map_err(|_| Error::Custom(format!("{i} out of i16 range")))?)
    }
    fn deserialize_i32<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let i = int_to_i128(self.stream.expect_next()?)?;
        v.visit_i32(i.try_into().map_err(|_| Error::Custom(format!("{i} out of i32 range")))?)
    }
    fn deserialize_i64<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let i = int_to_i128(self.stream.expect_next()?)?;
        v.visit_i64(i.try_into().map_err(|_| Error::Custom(format!("{i} out of i64 range")))?)
    }
    fn deserialize_i128<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let i = int_to_i128(self.stream.expect_next()?)?;
        v.visit_i128(i)
    }
    fn deserialize_u8<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let u = int_to_u128(self.stream.expect_next()?)?;
        v.visit_u8(u.try_into().map_err(|_| Error::Custom(format!("{u} out of u8 range")))?)
    }
    fn deserialize_u16<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let u = int_to_u128(self.stream.expect_next()?)?;
        v.visit_u16(u.try_into().map_err(|_| Error::Custom(format!("{u} out of u16 range")))?)
    }
    fn deserialize_u32<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let u = int_to_u128(self.stream.expect_next()?)?;
        v.visit_u32(u.try_into().map_err(|_| Error::Custom(format!("{u} out of u32 range")))?)
    }
    fn deserialize_u64<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        let u = int_to_u128(self.stream.expect_next()?)?;
        v.visit_u64(u.try_into().map_err(|_| Error::Custom(format!("{u} out of u64 range")))?)
    }
    fn deserialize_u128<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_u128(int_to_u128(self.stream.expect_next()?)?)
    }

    fn deserialize_f32<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_f32(float(self.stream.expect_next()?)? as f32)
    }
    fn deserialize_f64<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        v.visit_f64(float(self.stream.expect_next()?)?)
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        // Char values may arrive as either Token::Str (bracketed form
        // `[x]`) or Token::Ident (bare-identifier form for single-char
        // idents like `a`, `_`). `serialize_char` routes through
        // write_str_literal which emits bare when eligible — the
        // deserialize path must accept both.
        let (source, s) = match self.stream.expect_next()? {
            Token::Str(s) => ("string", s),
            Token::Ident(s) => ("identifier", s),
            other => return Err(Error::Custom(format!("expected string for char, got {other:?}"))),
        };
        let mut chars = s.chars();
        let c = chars.next().ok_or_else(|| Error::Custom(format!("empty {source} for char")))?;
        if chars.next().is_some() {
            return Err(Error::Custom(format!("expected single char, got {s:?}")));
        }
        visitor.visit_char(c)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.stream.expect_next()? {
            Token::Str(s) => visitor.visit_string(s),
            // Bare-identifier form: a schema expecting a string may
            // receive an ident-class token, which we treat as the
            // string content. Reserved keywords (`true`, `false`) are
            // already tokenised separately; `None` reaches here only
            // outside an `Option` context, where treating it as the
            // literal string "None" is correct.
            Token::Ident(s) => visitor.visit_string(s),
            other => Err(Error::Custom(format!("expected string, got {other:?}"))),
        }
    }
    fn deserialize_string<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        self.deserialize_str(v)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.stream.expect_next()? {
            Token::Bytes(b) => visitor.visit_byte_buf(b),
            other => Err(Error::Custom(format!("expected bytes, got {other:?}"))),
        }
    }
    fn deserialize_byte_buf<V: Visitor<'de>>(self, v: V) -> Result<V::Value> {
        self.deserialize_bytes(v)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.stream.peek()? {
            Some(Token::Ident(s)) if s == "None" => {
                let _ = self.stream.next()?;
                visitor.visit_none()
            }
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, _v: V) -> Result<V::Value> {
        Err(Error::UnitForbidden)
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        match self.stream.expect_next()? {
            Token::Ident(s) if s == name => visitor.visit_unit(),
            other => Err(Error::Custom(format!("expected unit struct `{name}`, got {other:?}"))),
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        // Newtype structs wrap: `(Name value)`.
        self.stream.expect_matching(&Token::LParen)?;
        match self.stream.expect_next()? {
            Token::Ident(s) if s == name => {}
            other => return Err(Error::Custom(format!(
                "expected newtype struct `{name}`, got {other:?}"
            ))),
        }
        let value = visitor.visit_newtype_struct(&mut *self)?;
        self.stream.expect_matching(&Token::RParen)?;
        Ok(value)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.stream.expect_matching(&Token::LAngle)?;
        let value = visitor.visit_seq(SeqReader { de: self })?;
        self.stream.expect_matching(&Token::RAngle)?;
        Ok(value)
    }

    fn deserialize_tuple<V: Visitor<'de>>(self, _len: usize, v: V) -> Result<V::Value> {
        self.deserialize_seq(v)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        len: usize,
        _visitor: V,
    ) -> Result<V::Value> {
        // Multi-field unnamed structs have no schema field names.
        // Single-field tuple structs go through deserialize_newtype_struct.
        Err(Error::MultiFieldTupleStructForbidden { name, len })
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.stream.expect_matching(&Token::LAngle)?;
        let value = visitor.visit_map(MapReader { de: self, done: false })?;
        self.stream.expect_matching(&Token::RAngle)?;
        Ok(value)
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        // Positional: field identities come from the Rust schema, not
        // the text. serde's derive-generated visitor accepts both
        // visit_seq (positional) and visit_map (named); we drive it
        // through visit_seq.
        self.stream.expect_matching(&Token::LParen)?;
        match self.stream.expect_next()? {
            Token::Ident(s) if s == name => {}
            other => return Err(Error::Custom(format!("expected struct `{name}`, got {other:?}"))),
        }
        let value = visitor.visit_seq(PositionalArgs { de: self })?;
        self.stream.expect_matching(&Token::RParen)?;
        Ok(value)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        match self.stream.peek()? {
            Some(Token::Ident(_)) => {
                // Bare PascalCase ident → unit variant.
                visitor.visit_enum(UnitVariant { de: self })
            }
            Some(Token::LParen) => {
                // (Variant ...) — payload-bearing variant.
                self.stream.next()?; // consume LParen
                visitor.visit_enum(PayloadVariant { de: self })
            }
            other => Err(Error::Custom(format!("expected enum variant, got {other:?}"))),
        }
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        match self.stream.expect_next()? {
            Token::Ident(s) => visitor.visit_string(s),
            other => Err(Error::Custom(format!("expected identifier, got {other:?}"))),
        }
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, _v: V) -> Result<V::Value> {
        Err(Error::Custom("ignored_any not supported".into()))
    }
}

// ---------- sequence reader ----------

struct SeqReader<'a, 'de> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> SeqAccess<'de> for SeqReader<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>> {
        if matches!(self.de.stream.peek()?, Some(Token::RAngle) | None) {
            return Ok(None);
        }
        seed.deserialize(&mut *self.de).map(Some)
    }
}

// ---------- positional args (for tuple structs / tuple variants) ----------

struct PositionalArgs<'a, 'de> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> SeqAccess<'de> for PositionalArgs<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>> {
        if matches!(self.de.stream.peek()?, Some(Token::RParen) | None) {
            return Ok(None);
        }
        seed.deserialize(&mut *self.de).map(Some)
    }
}

// ---------- map reader ----------

struct MapReader<'a, 'de> {
    de: &'a mut Deserializer<'de>,
    done: bool,
}

impl<'a, 'de> MapAccess<'de> for MapReader<'a, 'de> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>> {
        if self.done {
            return Ok(None);
        }
        match self.de.stream.peek()? {
            Some(Token::RAngle) | None => Ok(None),
            Some(Token::LParen) => {
                self.de.stream.next()?;
                seed.deserialize(&mut *self.de).map(Some)
            }
            other => Err(Error::Custom(format!(
                "expected `(` to open map entry, got {other:?}"
            ))),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value> {
        let value = seed.deserialize(&mut *self.de)?;
        self.de.stream.expect_matching(&Token::RParen)?;
        Ok(value)
    }
}

// ---------- enum access: unit variant (bare ident) ----------

struct UnitVariant<'a, 'de> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> EnumAccess<'de> for UnitVariant<'a, 'de> {
    type Error = Error;
    type Variant = UnitVariantAccess;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self::Variant)> {
        let name = match self.de.stream.expect_next()? {
            Token::Ident(s) => s,
            other => return Err(Error::Custom(format!("expected variant name, got {other:?}"))),
        };
        let v = seed.deserialize(name.into_deserializer())?;
        Ok((v, UnitVariantAccess))
    }
}

struct UnitVariantAccess;

impl<'de> VariantAccess<'de> for UnitVariantAccess {
    type Error = Error;

    fn unit_variant(self) -> Result<()> { Ok(()) }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, _seed: T) -> Result<T::Value> {
        Err(Error::Custom("expected payload-bearing variant, got bare unit variant".into()))
    }
    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, _v: V) -> Result<V::Value> {
        Err(Error::Custom("expected tuple variant, got bare unit variant".into()))
    }
    fn struct_variant<V: Visitor<'de>>(self, _fields: &'static [&'static str], _v: V) -> Result<V::Value> {
        Err(Error::Custom("expected struct variant, got bare unit variant".into()))
    }
}

// ---------- enum access: payload variant (LParen already consumed) ----------

struct PayloadVariant<'a, 'de> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> EnumAccess<'de> for PayloadVariant<'a, 'de> {
    type Error = Error;
    type Variant = PayloadVariantAccess<'a, 'de>;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self::Variant)> {
        let name = match self.de.stream.expect_next()? {
            Token::Ident(s) => s,
            other => return Err(Error::Custom(format!("expected variant name, got {other:?}"))),
        };
        let v = seed.deserialize(name.into_deserializer())?;
        Ok((v, PayloadVariantAccess { de: self.de }))
    }
}

struct PayloadVariantAccess<'a, 'de> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> VariantAccess<'de> for PayloadVariantAccess<'a, 'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        self.de.stream.expect_matching(&Token::RParen)?;
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value> {
        let value = seed.deserialize(&mut *self.de)?;
        self.de.stream.expect_matching(&Token::RParen)?;
        Ok(value)
    }

    fn tuple_variant<V: Visitor<'de>>(self, len: usize, _visitor: V) -> Result<V::Value> {
        // Multi-field unnamed variants have no schema field names.
        // Single-field variants go through newtype_variant_seed instead.
        Err(Error::MultiFieldTupleStructForbidden { name: "<tuple-variant>", len })
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        // Positional — same as deserialize_struct.
        let value = visitor.visit_seq(PositionalArgs { de: self.de })?;
        self.de.stream.expect_matching(&Token::RParen)?;
        Ok(value)
    }
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::from_str;
    use crate::ser::to_string;
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;

    fn roundtrip<T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug>(v: T) {
        let text = to_string(&v).expect("serialize");
        let back: T = from_str(&text).expect("deserialize");
        assert_eq!(back, v, "roundtrip mismatch; intermediate text was {text:?}");
    }

    #[test]
    fn primitives() {
        roundtrip(true);
        roundtrip(false);
        roundtrip(42i32);
        roundtrip(-7i64);
        roundtrip(0u32);
        roundtrip(2.5f64);
        roundtrip(1.0f64);
        roundtrip(-0.5f32);
        roundtrip("hello".to_string());
    }

    #[test]
    fn strings() {
        roundtrip("hello world".to_string());
        roundtrip("with ] bracket".to_string());
        roundtrip("multi\nline".to_string());
    }

    #[test]
    fn bytes() {
        // std Vec<u8> serializes as seq; use a wrapper that forces bytes.
        // Test via direct lexer+deserializer path:
        let text = "#a1b2c3";
        use serde::de::Deserializer as _;
        struct BytesVisitor;
        impl<'de> serde::de::Visitor<'de> for BytesVisitor {
            type Value = Vec<u8>;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "bytes")
            }
            fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Vec<u8>, E> { Ok(v) }
        }
        let mut de = super::Deserializer::new(text);
        let out = de.deserialize_bytes(BytesVisitor).unwrap();
        assert_eq!(out, vec![0xa1, 0xb2, 0xc3]);
    }

    #[test]
    fn option() {
        roundtrip::<Option<i32>>(None);
        roundtrip::<Option<i32>>(Some(7));
    }

    #[test]
    fn unit_struct() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Marker;
        roundtrip(Marker);
    }

    #[test]
    fn simple_enum() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        enum Status { Active, Archived }
        roundtrip(Status::Active);
        roundtrip(Status::Archived);
    }

    #[test]
    fn newtype_struct() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Id(u32);
        roundtrip(Id(42));
    }

    #[test]
    fn newtype_variant() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        enum E { V(i32), W(String) }
        roundtrip(E::V(7));
        roundtrip(E::W("hi".into()));
    }

    #[test]
    fn tuple_struct_ser_rejected() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Pair(i32, i32);
        // Nota forbids multi-field unnamed structs on both sides.
        assert!(to_string(&Pair(3, 4)).is_err());
        assert!(from_str::<Pair>("(Pair 3 4)").is_err());
    }

    #[test]
    fn tuple_variant_ser_rejected() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        enum E { Pair(i32, i32), Triple(i32, i32, i32) }
        assert!(to_string(&E::Pair(3, 4)).is_err());
        assert!(to_string(&E::Triple(1, 2, 3)).is_err());
        assert!(from_str::<E>("(Pair 3 4)").is_err());
    }

    #[test]
    fn seq() {
        roundtrip(vec![1, 2, 3]);
        roundtrip::<Vec<i32>>(vec![]);
    }

    #[test]
    fn tuple() {
        roundtrip((1i32, "a".to_string(), true));
    }

    #[test]
    fn struct_() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Point { horizontal: f64, vertical: f64 }
        roundtrip(Point { horizontal: 3.0, vertical: 4.0 });
    }

    #[test]
    fn struct_variant() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        enum Shape {
            Circle { radius: f64 },
            Square { side: f64 },
        }
        roundtrip(Shape::Circle { radius: 2.0 });
        roundtrip(Shape::Square { side: 3.5 });
    }

    #[test]
    fn nested_struct() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Point { x: f64, y: f64 }
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Line { start: Point, end: Point }
        roundtrip(Line {
            start: Point { x: 0.0, y: 0.0 },
            end: Point { x: 1.0, y: 2.0 },
        });
    }

    #[test]
    fn map() {
        let mut m = BTreeMap::new();
        m.insert("alpha".to_string(), 1);
        m.insert("beta".to_string(), 2);
        m.insert("gamma".to_string(), 3);
        roundtrip(m);
    }

    #[test]
    fn vec_of_structs() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Point { x: i32, y: i32 }
        roundtrip(vec![
            Point { x: 0, y: 0 },
            Point { x: 1, y: 1 },
            Point { x: -5, y: 10 },
        ]);
    }

    #[test]
    fn struct_with_option() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Config {
            name: String,
            port: Option<u16>,
            debug: bool,
        }
        roundtrip(Config { name: "server".into(), port: Some(8080), debug: true });
        roundtrip(Config { name: "server".into(), port: None, debug: false });
    }

    #[test]
    fn struct_with_vec_and_enum() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        enum Kind { Reader, Writer }
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Actor { name: String, kind: Kind, tags: Vec<String> }
        roundtrip(Actor {
            name: "alice".into(),
            kind: Kind::Reader,
            tags: vec!["fast".into(), "reliable".into()],
        });
    }

    #[test]
    fn ignores_comments() {
        #[derive(Deserialize, PartialEq, Debug)]
        struct Point { x: f64, y: f64 }
        let text = "(Point ;; comment\n  3.0 ;; inline\n  4.0)";
        let p: Point = from_str(text).unwrap();
        assert_eq!(p, Point { x: 3.0, y: 4.0 });
    }

    #[test]
    fn wrong_struct_name_fails() {
        #[derive(Deserialize, Debug)]
        #[allow(dead_code)]
        struct Point { x: f64, y: f64 }
        assert!(from_str::<Point>("(Line 1.0 2.0)").is_err());
    }

    #[test]
    fn integer_overflow_rejected() {
        let result: Result<i8, _> = from_str("200");
        assert!(result.is_err());
    }
}
