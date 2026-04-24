//! Tokenizer for nota text.
//!
//! Produces a stream of [`Token`]s. The parser is first-token-decidable
//! at every choice point — the lexer never needs schema information to
//! classify a token.

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    LParen,       // (
    RParen,       // )
    LAngle,       // <
    RAngle,       // >
    Equals,       // =
    Colon,        // :
    Bool(bool),
    /// Signed integer literal. Fits in `[i128::MIN, i128::MAX]`.
    Int(i128),
    /// Unsigned integer literal beyond `i128::MAX` and up to
    /// `u128::MAX`. Used only when the raw digits exceed `i128::MAX`
    /// — values that fit in `i128` are always tokenised as `Int`.
    UInt(u128),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    /// Bare identifier — the parser decides whether it's a type name,
    /// field name, tag, or a `true`/`false`/`None` keyword based on
    /// context.
    Ident(String),
}

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn peek_byte(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    fn peek2_bytes(&self) -> Option<(u8, u8)> {
        let b = self.input.as_bytes();
        if self.pos + 1 < b.len() {
            Some((b[self.pos], b[self.pos + 1]))
        } else {
            None
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek_byte() {
                Some(b) if b.is_ascii_whitespace() => { self.pos += 1; }
                Some(b';') if self.input.as_bytes().get(self.pos + 1) == Some(&b';') => {
                    // line comment to end of line
                    while let Some(b) = self.peek_byte() {
                        self.pos += 1;
                        if b == b'\n' { break; }
                    }
                }
                _ => break,
            }
        }
    }

    pub fn next_token(&mut self) -> Result<Option<Token>> {
        self.skip_whitespace_and_comments();
        let Some(b) = self.peek_byte() else { return Ok(None); };

        match b {
            b'(' => { self.pos += 1; Ok(Some(Token::LParen)) }
            b')' => { self.pos += 1; Ok(Some(Token::RParen)) }
            b'<' => { self.pos += 1; Ok(Some(Token::LAngle)) }
            b'>' => { self.pos += 1; Ok(Some(Token::RAngle)) }
            b'=' => { self.pos += 1; Ok(Some(Token::Equals)) }
            b':' => { self.pos += 1; Ok(Some(Token::Colon)) }
            b'[' => {
                self.pos += 1;
                if self.peek_byte() == Some(b'|') {
                    self.pos += 1;
                    let s = self.read_multiline_string()?;
                    Ok(Some(Token::Str(s)))
                } else {
                    let s = self.read_inline_string()?;
                    Ok(Some(Token::Str(s)))
                }
            }
            b'#' => {
                self.pos += 1;
                let bytes = self.read_bytes()?;
                Ok(Some(Token::Bytes(bytes)))
            }
            b'-' | b'0'..=b'9' => {
                let tok = self.read_number()?;
                Ok(Some(tok))
            }
            _ if is_ident_start(b) => {
                let ident = self.read_ident();
                match ident.as_str() {
                    "true" => Ok(Some(Token::Bool(true))),
                    "false" => Ok(Some(Token::Bool(false))),
                    _ => Ok(Some(Token::Ident(ident))),
                }
            }
            _ => Err(Error::Custom(format!(
                "unexpected character {:?} at byte offset {}",
                b as char, self.pos
            ))),
        }
    }

    fn read_inline_string(&mut self) -> Result<String> {
        let start = self.pos;
        while let Some(b) = self.peek_byte() {
            if b == b']' {
                let s = self.input[start..self.pos].to_string();
                self.pos += 1;
                return Ok(s);
            }
            if b == b'\n' {
                return Err(Error::Custom(
                    "unexpected newline in inline `[ ]` string — use `[| |]` for multiline".into(),
                ));
            }
            self.pos += 1;
        }
        Err(Error::Custom("unterminated inline string — missing `]`".into()))
    }

    fn read_multiline_string(&mut self) -> Result<String> {
        // Consume up to `|]`. Content starts right after `[|`.
        let start = self.pos;
        while let Some((a, b)) = self.peek2_bytes() {
            if a == b'|' && b == b']' {
                let raw = &self.input[start..self.pos];
                self.pos += 2;
                return Ok(dedent(raw));
            }
            self.pos += 1;
        }
        Err(Error::Custom("unterminated multiline string — missing `|]`".into()))
    }

    fn read_bytes(&mut self) -> Result<Vec<u8>> {
        let start = self.pos;
        while let Some(b) = self.peek_byte() {
            if b.is_ascii_hexdigit() && !b.is_ascii_uppercase() {
                self.pos += 1;
            } else {
                break;
            }
        }
        let hex = &self.input[start..self.pos];
        if hex.is_empty() {
            return Err(Error::Custom("`#` must be followed by hex digits".into()));
        }
        if hex.len() % 2 != 0 {
            return Err(Error::Custom(format!(
                "byte literal must have even number of hex digits, got {}",
                hex.len()
            )));
        }
        let mut out = Vec::with_capacity(hex.len() / 2);
        for chunk in hex.as_bytes().chunks(2) {
            let hi = hex_digit(chunk[0]).unwrap();
            let lo = hex_digit(chunk[1]).unwrap();
            out.push((hi << 4) | lo);
        }
        Ok(out)
    }

    fn read_number(&mut self) -> Result<Token> {
        let start = self.pos;
        if self.peek_byte() == Some(b'-') {
            self.pos += 1;
        }
        // look for prefixed integer form: 0x / 0b / 0o
        if self.peek_byte() == Some(b'0') {
            if let Some(&b) = self.input.as_bytes().get(self.pos + 1) {
                match b {
                    b'x' | b'X' => { self.pos += 2; return self.read_radix_int(start, 16); }
                    b'b' | b'B' => { self.pos += 2; return self.read_radix_int(start, 2); }
                    b'o' | b'O' => { self.pos += 2; return self.read_radix_int(start, 8); }
                    _ => {}
                }
            }
        }

        let mut saw_dot = false;
        let mut saw_exp = false;
        while let Some(b) = self.peek_byte() {
            match b {
                b'0'..=b'9' | b'_' => { self.pos += 1; }
                b'.' if !saw_dot && !saw_exp => { saw_dot = true; self.pos += 1; }
                b'e' | b'E' if !saw_exp => {
                    saw_exp = true;
                    self.pos += 1;
                    if matches!(self.peek_byte(), Some(b'+') | Some(b'-')) {
                        self.pos += 1;
                    }
                }
                _ => break,
            }
        }
        let raw = &self.input[start..self.pos];
        let cleaned: String = raw.chars().filter(|c| *c != '_').collect();
        if saw_dot || saw_exp {
            cleaned.parse::<f64>()
                .map(Token::Float)
                .map_err(|e| Error::Custom(format!("invalid float {raw:?}: {e}")))
        } else {
            parse_int_literal(&cleaned, 10)
                .map_err(|e| Error::Custom(format!("invalid integer {raw:?}: {e}")))
        }
    }

    fn read_radix_int(&mut self, start: usize, radix: u32) -> Result<Token> {
        let digits_start = self.pos;
        while let Some(b) = self.peek_byte() {
            if b == b'_' || is_radix_digit(b, radix) {
                self.pos += 1;
            } else {
                break;
            }
        }
        let raw = &self.input[digits_start..self.pos];
        let cleaned: String = raw.chars().filter(|c| *c != '_').collect();
        parse_int_literal(&cleaned, radix)
            .map_err(|e| Error::Custom(format!("invalid radix-{radix} int {:?}: {e}", &self.input[start..self.pos])))
    }

    fn read_ident(&mut self) -> String {
        let start = self.pos;
        while let Some(b) = self.peek_byte() {
            if is_ident_continue(b) {
                self.pos += 1;
            } else {
                break;
            }
        }
        self.input[start..self.pos].to_string()
    }
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

fn is_radix_digit(b: u8, radix: u32) -> bool {
    match radix {
        2 => matches!(b, b'0' | b'1'),
        8 => matches!(b, b'0'..=b'7'),
        16 => b.is_ascii_hexdigit(),
        _ => false,
    }
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    }
}

/// Parse a numeric literal (digits plus optional leading `-`) as `i128`
/// if it fits; otherwise fall back to `u128` for the range
/// `(i128::MAX, u128::MAX]`. Returns a `Token` carrying whichever
/// representation preserves the value exactly.
fn parse_int_literal(cleaned: &str, radix: u32) -> std::result::Result<Token, std::num::ParseIntError> {
    match i128::from_str_radix(cleaned, radix) {
        Ok(i) => Ok(Token::Int(i)),
        Err(_) => {
            // i128 overflow — try u128. This only succeeds for
            // non-negative values above i128::MAX, so we preserve
            // signed-vs-unsigned semantics and don't accidentally
            // treat a user's `-5` as u128::MAX - 4.
            u128::from_str_radix(cleaned, radix).map(Token::UInt)
        }
    }
}

/// Strip the common leading-whitespace prefix from every non-empty line.
/// Leading and trailing empty lines are removed.
fn dedent(raw: &str) -> String {
    let lines: Vec<&str> = raw.lines().collect();
    let nonempty: Vec<&&str> = lines.iter().filter(|l| !l.trim().is_empty()).collect();
    let min_indent = nonempty.iter()
        .map(|l| l.bytes().take_while(|b| *b == b' ' || *b == b'\t').count())
        .min()
        .unwrap_or(0);

    let mut out = String::new();
    // Skip leading and trailing blank lines.
    let Some(start) = lines.iter().position(|l| !l.trim().is_empty()) else {
        // All blank — content is empty after dedent.
        return out;
    };
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map(|i| i + 1)
        .unwrap_or(lines.len());

    for (i, line) in lines[start..end].iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if line.len() >= min_indent {
            out.push_str(&line[min_indent..]);
        } else {
            out.push_str(line);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(s: &str) -> Vec<Token> {
        let mut l = Lexer::new(s);
        let mut out = Vec::new();
        while let Some(t) = l.next_token().unwrap() {
            out.push(t);
        }
        out
    }

    #[test]
    fn delimiters() {
        assert_eq!(lex("()<>="), vec![
            Token::LParen, Token::RParen, Token::LAngle, Token::RAngle, Token::Equals,
        ]);
    }

    #[test]
    fn bools_and_idents() {
        assert_eq!(lex("true false None foo-bar BazQux"), vec![
            Token::Bool(true),
            Token::Bool(false),
            Token::Ident("None".into()),
            Token::Ident("foo-bar".into()),
            Token::Ident("BazQux".into()),
        ]);
    }

    #[test]
    fn integers() {
        assert_eq!(lex("42 -7 0 1_000_000 0xff 0b1010 0o755"), vec![
            Token::Int(42),
            Token::Int(-7),
            Token::Int(0),
            Token::Int(1_000_000),
            Token::Int(0xff),
            Token::Int(0b1010),
            Token::Int(0o755),
        ]);
    }

    #[test]
    fn floats() {
        let toks = lex("2.5 -0.5 1.0 2e3 2.5e-1");
        match (&toks[0], &toks[1], &toks[2], &toks[3], &toks[4]) {
            (Token::Float(a), Token::Float(b), Token::Float(c), Token::Float(d), Token::Float(e)) => {
                assert!((a - 2.5).abs() < 1e-9);
                assert!((b - (-0.5)).abs() < 1e-9);
                assert!((c - 1.0).abs() < 1e-9);
                assert!((d - 2000.0).abs() < 1e-9);
                assert!((e - 0.25).abs() < 1e-9);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn inline_string() {
        assert_eq!(lex("[hello world]"), vec![Token::Str("hello world".into())]);
    }

    #[test]
    fn multiline_string_dedents() {
        let text = "[|\n    first line\n      indented\n    last line\n|]";
        let toks = lex(text);
        assert_eq!(toks, vec![Token::Str("first line\n  indented\nlast line".into())]);
    }

    #[test]
    fn bytes_literal() {
        assert_eq!(lex("#a1b2c3"), vec![Token::Bytes(vec![0xa1, 0xb2, 0xc3])]);
    }

    #[test]
    fn odd_length_bytes_rejected() {
        let mut l = Lexer::new("#abc");
        assert!(l.next_token().is_err());
    }

    #[test]
    fn comments_skipped() {
        assert_eq!(lex(";; comment\n42 ;; trailing\n7"), vec![
            Token::Int(42),
            Token::Int(7),
        ]);
    }

    #[test]
    fn record_tokens() {
        assert_eq!(lex("(Point x=3.0 y=4.0)"), vec![
            Token::LParen,
            Token::Ident("Point".into()),
            Token::Ident("x".into()),
            Token::Equals,
            Token::Float(3.0),
            Token::Ident("y".into()),
            Token::Equals,
            Token::Float(4.0),
            Token::RParen,
        ]);
    }
}
