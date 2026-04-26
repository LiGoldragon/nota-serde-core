//! Deserializer: parse nota/nexus text into types implementing [`serde::Deserialize`].
//!
//! Token-stream-driven recursive descent. The lexer produces tokens; the
//! Deserializer peeks and consumes them driven by the visitor's demands.
//!
//! [`Dialect`] selects the grammar. [`Dialect::Nexus`] enables
//! sentinel-name dispatch on [`BIND_SENTINEL`] / [`MUTATE_SENTINEL`] /
//! [`NEGATE_SENTINEL`] / [`VALIDATE_SENTINEL`] / [`SUBSCRIBE_SENTINEL`] /
//! [`ATOMIC_BATCH_SENTINEL`] newtype-struct names for the six
//! query-layer wrappers.

use serde::de::{
    self, DeserializeSeed, EnumAccess, IntoDeserializer, MapAccess, SeqAccess,
    VariantAccess, Visitor,
};

use crate::error::{Error, Result};
use crate::lexer::{is_pascal_case, Dialect, Lexer, Token};
use crate::ser::{
    ATOMIC_BATCH_SENTINEL, BIND_SENTINEL, MUTATE_SENTINEL, NEGATE_SENTINEL,
    SUBSCRIBE_SENTINEL, VALIDATE_SENTINEL,
};

pub fn from_str<'a, T: de::Deserialize<'a>>(input: &'a str) -> Result<T> {
    from_str_with(input, Dialect::Nota)
}

pub fn from_str_nexus<'a, T: de::Deserialize<'a>>(input: &'a str) -> Result<T> {
    from_str_with(input, Dialect::Nexus)
}

pub fn from_str_with<'a, T: de::Deserialize<'a>>(
    input: &'a str,
    dialect: Dialect,
) -> Result<T> {
    let mut de = Deserializer::with_dialect(input, dialect);
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
    fn new(input: &'a str, dialect: Dialect) -> Self {
        Self { lexer: Lexer::with_dialect(input, dialect), peeked: None }
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

    /// Consume the next token as a PascalCase ident (used for record /
    /// unit struct / variant heads). Returns the ident string. Rejects
    /// non-ident tokens and ident tokens whose first char is not ASCII
    /// uppercase. `kind` describes the position for the error message
    /// (`"struct"`, `"unit struct"`, `"newtype"`, `"variant"`).
    fn expect_pascal_head(&mut self, kind: &'static str) -> Result<String> {
        match self.expect_next()? {
            Token::Ident(s) => {
                if !is_pascal_case(&s) {
                    return Err(Error::Custom(format!(
                        "{kind} name must be PascalCase (first char uppercase ASCII); got `{s}`. Type and variant names follow PascalCase; bare lowercase identifiers in head position are forbidden."
                    )));
                }
                Ok(s)
            }
            other => Err(Error::Custom(format!(
                "expected {kind} name (PascalCase identifier), got {other:?}"
            ))),
        }
    }
}

impl<'de> Deserializer<'de> {
    pub fn new(input: &'de str) -> Self {
        Self::with_dialect(input, Dialect::Nota)
    }

    pub fn nexus(input: &'de str) -> Self {
        Self::with_dialect(input, Dialect::Nexus)
    }

    pub fn with_dialect(input: &'de str, dialect: Dialect) -> Self {
        Self { stream: TokenStream::new(input, dialect) }
    }

    pub fn dialect(&self) -> Dialect {
        self.stream.lexer.dialect()
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
        // Char values may arrive as either Token::Str (quoted form
        // `"x"`) or Token::Ident (bare-identifier form for single-char
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
        let head = self.stream.expect_pascal_head("unit struct")?;
        if head != name {
            return Err(Error::Custom(format!("expected unit struct `{name}`, got `{head}`")));
        }
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value> {
        if matches!(
            name,
            BIND_SENTINEL
                | MUTATE_SENTINEL
                | NEGATE_SENTINEL
                | VALIDATE_SENTINEL
                | SUBSCRIBE_SENTINEL
                | ATOMIC_BATCH_SENTINEL
        ) {
            if self.dialect() != Dialect::Nexus {
                return Err(Error::Custom(format!(
                    "sentinel newtype-struct `{name}` is not valid in nota dialect; deserialize via nexus (`from_str_nexus`) instead"
                )));
            }
            return match name {
                BIND_SENTINEL => {
                    self.stream.expect_matching(&Token::At)?;
                    match self.stream.expect_next()? {
                        Token::Ident(s) => {
                            let de: serde::de::value::StringDeserializer<Error> =
                                s.into_deserializer();
                            visitor.visit_newtype_struct(de)
                        }
                        other => Err(Error::Custom(format!(
                            "expected identifier after `@`, got {other:?}"
                        ))),
                    }
                }
                MUTATE_SENTINEL => {
                    self.stream.expect_matching(&Token::Tilde)?;
                    visitor.visit_newtype_struct(self)
                }
                NEGATE_SENTINEL => {
                    self.stream.expect_matching(&Token::Bang)?;
                    visitor.visit_newtype_struct(self)
                }
                VALIDATE_SENTINEL => {
                    self.stream.expect_matching(&Token::Question)?;
                    visitor.visit_newtype_struct(self)
                }
                SUBSCRIBE_SENTINEL => {
                    self.stream.expect_matching(&Token::Star)?;
                    visitor.visit_newtype_struct(self)
                }
                ATOMIC_BATCH_SENTINEL => {
                    self.stream.expect_matching(&Token::LBracketPipe)?;
                    let value = visitor.visit_newtype_struct(AtomicBatchInner { de: self })?;
                    self.stream.expect_matching(&Token::RBracketPipe)?;
                    Ok(value)
                }
                _ => unreachable!(),
            };
        }
        // Plain newtype — `(Name value)`.
        self.stream.expect_matching(&Token::LParen)?;
        let head = self.stream.expect_pascal_head("newtype")?;
        if head != name {
            return Err(Error::Custom(format!(
                "expected newtype struct `{name}`, got `{head}`"
            )));
        }
        let value = visitor.visit_newtype_struct(&mut *self)?;
        self.stream.expect_matching(&Token::RParen)?;
        Ok(value)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.stream.expect_matching(&Token::LBracket)?;
        let value = visitor.visit_seq(SeqReader { de: self })?;
        self.stream.expect_matching(&Token::RBracket)?;
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
        Err(Error::MultiFieldTupleStructForbidden { name, len })
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        self.stream.expect_matching(&Token::LBracket)?;
        let value = visitor.visit_map(MapReader { de: self, done: false })?;
        self.stream.expect_matching(&Token::RBracket)?;
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
        let head = self.stream.expect_pascal_head("struct")?;
        if head != name {
            return Err(Error::Custom(format!("expected struct `{name}`, got `{head}`")));
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
                visitor.visit_enum(UnitVariant { de: self })
            }
            Some(Token::LParen) => {
                self.stream.next()?;
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
        if matches!(self.de.stream.peek()?, Some(Token::RBracket) | None) {
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
            Some(Token::RBracket) | None => Ok(None),
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
        let name = self.de.stream.expect_pascal_head("variant")?;
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
        let name = self.de.stream.expect_pascal_head("variant")?;
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
        Err(Error::MultiFieldTupleStructForbidden { name: "<tuple-variant>", len })
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value> {
        let value = visitor.visit_seq(PositionalArgs { de: self.de })?;
        self.de.stream.expect_matching(&Token::RParen)?;
        Ok(value)
    }
}

// ---------- atomic-batch inner ----------

/// Deserializer wrapper presented to a sentinel `AtomicBatch<T>` newtype's
/// inner. Translates calls for sequences (`deserialize_seq`,
/// `deserialize_tuple`) into a stream of items between the already-consumed
/// `[|` and the `|]` we expect at end. Other ser/de surface delegates back
/// to the parent deserializer.
struct AtomicBatchInner<'a, 'de> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> de::Deserializer<'de> for AtomicBatchInner<'a, 'de> {
    type Error = Error;

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value> {
        visitor.visit_seq(AtomicBatchReader { de: self.de })
    }

    fn deserialize_tuple<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value> {
        self.deserialize_seq(visitor)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64
        char str string bytes byte_buf option unit unit_struct
        newtype_struct tuple_struct map struct enum identifier
        ignored_any
    }

    fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value> {
        Err(Error::Custom(
            "AtomicBatch inner must be a sequence type — `deserialize_any` not supported".into(),
        ))
    }
}

struct AtomicBatchReader<'a, 'de> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> SeqAccess<'de> for AtomicBatchReader<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>> {
        if matches!(self.de.stream.peek()?, Some(Token::RBracketPipe) | None) {
            return Ok(None);
        }
        seed.deserialize(&mut *self.de).map(Some)
    }
}
