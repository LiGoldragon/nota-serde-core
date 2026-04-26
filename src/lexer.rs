//! Tokenizer for nota/nexus text.
//!
//! Produces a stream of [`Token`]s. The parser is first-token-decidable
//! at every choice point — the lexer never needs schema information to
//! classify a token.
//!
//! [`Dialect`] selects the grammar. `Dialect::Nota` accepts only nota's
//! two delimiter pairs (`( )` records, `[ ]` sequences), two string
//! forms (`" "` inline, `""" """` multiline), and two sigils (`;;`,
//! `#`). `Dialect::Nexus` additionally accepts the messaging superset:
//! four extra delimiter pairs (`(| |)` patterns, `[| |]` atomic
//! batches, `{ }` shapes, `{| |}` constraints) plus the five sigils
//! `~`, `!`, `?`, `*`, `@` and the `=` bind-alias token. See
//! [nexus/spec/grammar.md](https://github.com/LiGoldragon/nexus/blob/main/spec/grammar.md)
//! for the locked token table.
//!
//! Reserved tokens — `<`, `>`, `<=`, `>=`, `!=`, and `=` outside the
//! `@a=@b` bind-alias position — are reserved for future comparison
//! operator design; the lexer rejects them in both dialects.

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
    LParen,    // (
    RParen,    // )
    LBracket,  // [
    RBracket,  // ]
    Equals,    // =
    Colon,     // :
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
    Question,     // ? (validate prefix)
    Star,         // * (subscribe prefix)
    LBrace,       // { (shape open)
    RBrace,       // } (shape close)
    LBracePipe,   // {| (constrain open)
    RBracePipe,   // |} (constrain close)
    LParenPipe,   // (| (pattern open)
    RParenPipe,   // |) (pattern close)
    LBracketPipe, // [| (atomic batch open)
    RBracketPipe, // |] (atomic batch close)
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

    fn peek_byte_at(&self, offset: usize) -> Option<u8> {
        self.input.as_bytes().get(self.pos + offset).copied()
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
            b'[' => Ok(Some(self.read_left_bracket())),
            b']' => { self.pos += 1; Ok(Some(Token::RBracket)) }
            b'<' | b'>' => Err(Error::Custom(format!(
                "reserved token {:?} at byte offset {} — `<` `>` `<=` `>=` `!=` are reserved for comparison operators (design pending)",
                b as char, self.pos,
            ))),
            b'=' => { self.pos += 1; Ok(Some(Token::Equals)) }
            b':' => { self.pos += 1; Ok(Some(Token::Colon)) }
            b'"' => Ok(Some(self.read_string()?)),
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
            b'?' if self.dialect == Dialect::Nexus => { self.pos += 1; Ok(Some(Token::Question)) }
            b'*' if self.dialect == Dialect::Nexus => { self.pos += 1; Ok(Some(Token::Star)) }

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

    /// `(` → `LParen`, `(|` → `LParenPipe` (nexus-only).
    fn read_left_paren(&mut self) -> Token {
        self.pos += 1;
        if self.dialect != Dialect::Nexus || self.peek_byte() != Some(b'|') {
            return Token::LParen;
        }
        self.pos += 1;
        Token::LParenPipe
    }

    /// `[` → `LBracket`, `[|` → `LBracketPipe` (nexus-only).
    fn read_left_bracket(&mut self) -> Token {
        self.pos += 1;
        if self.dialect != Dialect::Nexus || self.peek_byte() != Some(b'|') {
            return Token::LBracket;
        }
        self.pos += 1;
        Token::LBracketPipe
    }

    /// `{` → `LBrace`, `{|` → `LBracePipe`. Nexus-only; caller gates
    /// dialect.
    fn read_left_brace(&mut self) -> Token {
        self.pos += 1;
        if self.peek_byte() != Some(b'|') {
            return Token::LBrace;
        }
        self.pos += 1;
        Token::LBracePipe
    }

    /// Decode a leading `|` into its closing-pair token. Nexus-only.
    /// Closers: `|)` for patterns, `|}` for constraints, `|]` for
    /// atomic batches.
    fn read_pipe_close(&mut self) -> Result<Token> {
        self.pos += 1;
        match self.peek_byte() {
            Some(b')') => { self.pos += 1; Ok(Token::RParenPipe) }
            Some(b'}') => { self.pos += 1; Ok(Token::RBracePipe) }
            Some(b']') => { self.pos += 1; Ok(Token::RBracketPipe) }
            Some(other) => Err(Error::Custom(format!(
                "unexpected `|` followed by {:?} — expected `|)`, `|}}`, or `|]`",
                other as char
            ))),
            None => Err(Error::Custom("unexpected `|` at end of input".into())),
        }
    }

    /// Read a quoted string. `"` → inline; `"""` → multiline. Caller
    /// has positioned `self.pos` at the opening `"`.
    fn read_string(&mut self) -> Result<Token> {
        // Detect triple-quote.
        if self.peek_byte_at(1) == Some(b'"') && self.peek_byte_at(2) == Some(b'"') {
            self.pos += 3;
            self.read_multiline_string().map(Token::Str)
        } else {
            self.pos += 1;
            self.read_inline_string().map(Token::Str)
        }
    }

    /// Read an inline `" "` string. Pos is past the opening quote.
    /// Supports `\\`, `\"`, `\n`, `\t`, `\r` escapes; rejects bare
    /// newlines (use `""" """` for multiline content).
    fn read_inline_string(&mut self) -> Result<String> {
        let mut out = String::new();
        loop {
            let Some(b) = self.peek_byte() else {
                return Err(Error::Custom("unterminated inline string — missing `\"`".into()));
            };
            match b {
                b'"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                b'\n' => {
                    return Err(Error::Custom(
                        "unexpected newline in inline `\" \"` string — use `\"\"\" \"\"\"` for multiline".into(),
                    ));
                }
                b'\\' => {
                    let Some(esc) = self.peek_byte_at(1) else {
                        return Err(Error::Custom("unterminated escape at end of input".into()));
                    };
                    let translated = match esc {
                        b'\\' => '\\',
                        b'"' => '"',
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        other => return Err(Error::Custom(format!(
                            "unknown escape `\\{}` in inline string — supported: `\\\\`, `\\\"`, `\\n`, `\\t`, `\\r`",
                            other as char
                        ))),
                    };
                    out.push(translated);
                    self.pos += 2;
                }
                _ => {
                    // Read one UTF-8 codepoint to avoid splitting a
                    // multi-byte char.
                    let ch_start = self.pos;
                    let ch_len = utf8_char_len(b);
                    if ch_len > 1 {
                        // Bounds-check + push the whole char.
                        if self.pos + ch_len > self.input.len() {
                            return Err(Error::Custom(
                                "truncated UTF-8 sequence in inline string".into(),
                            ));
                        }
                        let s = &self.input[ch_start..ch_start + ch_len];
                        out.push_str(s);
                        self.pos += ch_len;
                    } else {
                        out.push(b as char);
                        self.pos += 1;
                    }
                }
            }
        }
    }

    /// Read a multiline `""" """` string. Pos is past the opening
    /// triple-quote. Contents are verbatim (no escape processing) and
    /// auto-dedented (strip common leading-whitespace prefix).
    fn read_multiline_string(&mut self) -> Result<String> {
        let start = self.pos;
        loop {
            let Some(b) = self.peek_byte() else {
                return Err(Error::Custom(
                    "unterminated multiline string — missing `\"\"\"`".into(),
                ));
            };
            if b == b'"'
                && self.peek_byte_at(1) == Some(b'"')
                && self.peek_byte_at(2) == Some(b'"')
            {
                let raw = &self.input[start..self.pos];
                self.pos += 3;
                return Ok(dedent(raw));
            }
            self.pos += 1;
        }
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

/// Number of bytes in the UTF-8 encoding starting with this leading
/// byte. Returns 1 for ASCII (also for invalid leading bytes — caller
/// re-checks bounds and `from_utf8` defers to the str slice for
/// validation).
fn utf8_char_len(b: u8) -> usize {
    match b {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
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
