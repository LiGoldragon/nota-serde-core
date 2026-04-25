//! Tokenizer for nota/nexus text.
//!
//! Produces a stream of [`Token`]s. The parser is first-token-decidable
//! at every choice point — the lexer never needs schema information to
//! classify a token.
//!
//! [`Dialect`] selects the grammar. `Dialect::Nota` accepts only nota's
//! four delimiter pairs and two sigils. `Dialect::Nexus` additionally
//! accepts the nexus superset: three additional delimiter pairs plus
//! the three sigils `~`, `@`, `!`; the `=` bind-alias token; and the
//! Tier-1 extensions from [reports/013](../../../../../mentci/reports/013-nexus-syntax-proposal.md):
//! `<| |>` stream, `(|| ||)` optional pattern, `{|| ||}` atomic txn.

use crate::error::{Error, Result};

/// Grammar dialect — picks the token set the lexer recognises.
///
/// [`Dialect::Nota`] is the strict data-layer subset. [`Dialect::Nexus`]
/// is the messaging-layer superset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dialect {
    #[default]
    Nota,
    Nexus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Shared with both dialects.
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

    // Nexus-dialect-only tokens. Never produced in Nota mode.
    Tilde,        // ~ (mutate prefix)
    At,           // @ (bind prefix)
    Bang,         // ! (negate prefix)
    LBrace,       // { (shape open)
    RBrace,       // } (shape close)
    LBracePipe,   // {| (constrain open)
    RBracePipe,   // |} (constrain close)
    LParenPipe,   // (| (pattern open)
    RParenPipe,   // |) (pattern close)

    // Tier-1 tokens (report 013). Never produced in Nota mode.
    LAnglePipe,   // <| (stream / subscription open)
    RAnglePipe,   // |> (stream close)
    LParenDouble, // (|| (optional pattern open)
    RParenDouble, // ||) (optional pattern close)
    LBraceDouble, // {|| (atomic transaction open)
    RBraceDouble, // ||} (atomic transaction close)
    LAngleDouble, // <|| (windowed stream open, Phase 2 reserved)
    RAngleDouble, // ||> (windowed stream close, Phase 2 reserved)
}

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
    dialect: Dialect,
}

impl<'a> Lexer<'a> {
    /// Create a lexer in the default (Nota) dialect.
    pub fn new(input: &'a str) -> Self {
        Self::with_dialect(input, Dialect::Nota)
    }

    /// Create a lexer for the nexus superset.
    pub fn nexus(input: &'a str) -> Self {
        Self::with_dialect(input, Dialect::Nexus)
    }

    pub fn with_dialect(input: &'a str, dialect: Dialect) -> Self {
        Self { input, pos: 0, dialect }
    }

    pub fn dialect(&self) -> Dialect {
        self.dialect
    }

    fn peek_byte(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.input.as_bytes().get(self.pos + offset).copied()
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
            b'(' => Ok(Some(self.read_left_paren())),
            b')' => { self.pos += 1; Ok(Some(Token::RParen)) }
            b'<' => Ok(Some(self.read_left_angle())),
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
            b'-' | b'0'..=b'9' => Ok(Some(self.read_number()?)),

            // Nexus-dialect sigils + delimiters.
            b'{' if self.dialect == Dialect::Nexus => Ok(Some(self.read_left_brace())),
            b'}' if self.dialect == Dialect::Nexus => { self.pos += 1; Ok(Some(Token::RBrace)) }
            b'|' if self.dialect == Dialect::Nexus => self.read_pipe_close().map(Some),
            b'~' if self.dialect == Dialect::Nexus => { self.pos += 1; Ok(Some(Token::Tilde)) }
            b'@' if self.dialect == Dialect::Nexus => { self.pos += 1; Ok(Some(Token::At)) }
            b'!' if self.dialect == Dialect::Nexus => { self.pos += 1; Ok(Some(Token::Bang)) }

            _ if is_ident_start(b) => {
                let ident = self.read_ident();
                match ident.as_str() {
                    "true" => Ok(Some(Token::Bool(true))),
                    "false" => Ok(Some(Token::Bool(false))),
                    _ => Ok(Some(Token::Ident(ident))),
                }
            }
            _ => Err(Error::Custom(format!(
                "unexpected character {:?} at byte offset {} ({} dialect)",
                b as char, self.pos,
                match self.dialect { Dialect::Nota => "nota", Dialect::Nexus => "nexus" }
            ))),
        }
    }

    /// `(` → `LParen`, `(|` → `LParenPipe`, `(||` → `LParenDouble`
    /// (the last two nexus-only).
    fn read_left_paren(&mut self) -> Token {
        self.pos += 1;
        if self.dialect != Dialect::Nexus || self.peek_byte() != Some(b'|') {
            return Token::LParen;
        }
        self.pos += 1;
        if self.peek_byte() == Some(b'|') {
            self.pos += 1;
            Token::LParenDouble
        } else {
            Token::LParenPipe
        }
    }

    /// `{` → `LBrace`, `{|` → `LBracePipe`, `{||` → `LBraceDouble`.
    /// Nexus-only; caller gates dialect.
    fn read_left_brace(&mut self) -> Token {
        self.pos += 1;
        if self.peek_byte() != Some(b'|') {
            return Token::LBrace;
        }
        self.pos += 1;
        if self.peek_byte() == Some(b'|') {
            self.pos += 1;
            Token::LBraceDouble
        } else {
            Token::LBracePipe
        }
    }

    /// `<` → `LAngle`, `<|` → `LAnglePipe`, `<||` → `LAngleDouble`
    /// (the piped forms nexus-only).
    fn read_left_angle(&mut self) -> Token {
        self.pos += 1;
        if self.dialect != Dialect::Nexus || self.peek_byte() != Some(b'|') {
            return Token::LAngle;
        }
        self.pos += 1;
        if self.peek_byte() == Some(b'|') {
            self.pos += 1;
            Token::LAngleDouble
        } else {
            Token::LAnglePipe
        }
    }

    /// Decode a leading `|` into its closing-pair token. Nexus-only.
    ///
    /// Single-pipe closers: `|)`, `|}`, `|>`.
    /// Double-pipe closers: `||)`, `||}`, `||>`.
    fn read_pipe_close(&mut self) -> Result<Token> {
        self.pos += 1;
        if self.peek_byte() == Some(b'|') {
            // double-pipe close
            let tail = self.peek_at(1);
            match tail {
                Some(b')') => { self.pos += 2; Ok(Token::RParenDouble) }
                Some(b'}') => { self.pos += 2; Ok(Token::RBraceDouble) }
                Some(b'>') => { self.pos += 2; Ok(Token::RAngleDouble) }
                Some(other) => Err(Error::Custom(format!(
                    "unexpected `||` followed by {:?} — expected `||)`, `||}}` or `||>`",
                    other as char
                ))),
                None => Err(Error::Custom("unexpected `||` at end of input".into())),
            }
        } else {
            match self.peek_byte() {
                Some(b')') => { self.pos += 1; Ok(Token::RParenPipe) }
                Some(b'}') => { self.pos += 1; Ok(Token::RBracePipe) }
                Some(b'>') => { self.pos += 1; Ok(Token::RAnglePipe) }
                Some(other) => Err(Error::Custom(format!(
                    "unexpected `|` followed by {:?} — expected `|)`, `|}}`, `|>`, `||)`, `||}}` or `||>`",
                    other as char
                ))),
                None => Err(Error::Custom("unexpected `|` at end of input".into())),
            }
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
        Err(_) => u128::from_str_radix(cleaned, radix).map(Token::UInt),
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
    let Some(start) = lines.iter().position(|l| !l.trim().is_empty()) else {
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
