//! Tokens for the type stripper. The lexer reads one token at a time, on
//! the parser's request, because only the parser knows whether a `/` starts
//! a regular expression, a `}` resumes a template or a `>>` closes two type
//! argument lists.

use super::Unparsed;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// An identifier, keyword or `#private` name.
    Word,
    Number,
    String,
    /// A piece of a template literal, from its backtick or `}` up to the
    /// next `${` or, when `tail`, the closing backtick.
    Template {
        tail: bool,
    },
    Punctuator,
    End,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Token {
    pub kind: Kind,
    pub start: usize,
    pub end: usize,
    /// Whether a line break separates this token from the one before it.
    pub newline_before: bool,
}

/// Longest first, so the first match is the whole punctuator.
const PUNCTUATORS: &[&str] = &[
    ">>>=", "...", "===", "!==", "**=", "<<=", ">>=", ">>>", "&&=", "||=", "??=", "=>", "==", "!=",
    "<=", ">=", "&&", "||", "??", "?.", "++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=",
    "<<", ">>", "**", "{", "}", "(", ")", "[", "]", ";", ",", "<", ">", "+", "-", "*", "/", "%",
    "&", "|", "^", "!", "~", "?", ":", "=", ".", "@",
];

pub(super) struct Lexer<'a> {
    source: &'a str,
    /// Where the next token is read from.
    pub position: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        // A hashbang line is a comment.
        let position = if source.starts_with("#!") {
            source.find('\n').unwrap_or(source.len())
        } else {
            0
        };
        Self { source, position }
    }

    fn byte(&self, at: usize) -> u8 {
        self.source.as_bytes().get(at).copied().unwrap_or(0)
    }

    fn rest(&self) -> &'a str {
        &self.source[self.position..]
    }

    pub fn next(&mut self) -> Result<Token, Unparsed> {
        let newline_before = self.skip_trivia()?;
        let start = self.position;
        let kind = self.scan()?;
        Ok(Token {
            kind,
            start,
            end: self.position,
            newline_before,
        })
    }

    /// Skips whitespace and comments, returning whether a line break was
    /// among them.
    fn skip_trivia(&mut self) -> Result<bool, Unparsed> {
        let mut newline = false;
        loop {
            let rest = self.rest();
            let Some(character) = rest.chars().next() else {
                return Ok(newline);
            };
            if is_line_break(character) {
                newline = true;
                self.position += character.len_utf8();
            } else if character.is_whitespace() || character == '\u{feff}' {
                self.position += character.len_utf8();
            } else if rest.starts_with("//") {
                let length = rest.find(is_line_break).unwrap_or(rest.len());
                self.position += length;
            } else if let Some(comment) = rest.strip_prefix("/*") {
                let length = comment.find("*/").ok_or(Unparsed)? + 4;
                newline |= rest[..length].contains(is_line_break);
                self.position += length;
            } else {
                return Ok(newline);
            }
        }
    }

    fn scan(&mut self) -> Result<Kind, Unparsed> {
        let rest = self.rest();
        let Some(character) = rest.chars().next() else {
            return Ok(Kind::End);
        };
        let next_is_digit = self.byte(self.position + 1).is_ascii_digit();
        if character.is_ascii_digit() || (character == '.' && next_is_digit) {
            self.scan_number();
            return Ok(Kind::Number);
        }
        if character == '"' || character == '\'' {
            self.scan_string(character)?;
            return Ok(Kind::String);
        }
        if character == '`' {
            self.position += 1;
            return self.scan_template();
        }
        if character == '#' || is_word_character(character) {
            self.position += character.len_utf8();
            self.scan_word();
            return Ok(Kind::Word);
        }
        for punctuator in PUNCTUATORS {
            if rest.starts_with(punctuator) {
                // `a?.5:b` is a conditional, not an optional chain.
                if *punctuator == "?." && self.byte(self.position + 2).is_ascii_digit() {
                    continue;
                }
                self.position += punctuator.len();
                return Ok(Kind::Punctuator);
            }
        }
        Err(Unparsed)
    }

    fn scan_word(&mut self) {
        while let Some(character) = self.rest().chars().next() {
            if !is_word_character(character) {
                break;
            }
            self.position += character.len_utf8();
            // The braces of a `\u{…}` escape belong to the word.
            if character == '\\' && self.rest().starts_with("u{") {
                let length = self.rest().find('}').map_or(1, |close| close + 1);
                self.position += length;
            }
        }
    }

    /// Reads a numeric literal loosely: QuickJS judges whether it is one.
    fn scan_number(&mut self) {
        let prefixed = self.byte(self.position) == b'0'
            && matches!(
                self.byte(self.position + 1),
                b'x' | b'X' | b'o' | b'O' | b'b' | b'B'
            );
        if !prefixed {
            self.skip_digits();
            if self.byte(self.position) == b'.' {
                self.position += 1;
                self.skip_digits();
            }
            if matches!(self.byte(self.position), b'e' | b'E') {
                let sign = matches!(self.byte(self.position + 1), b'+' | b'-');
                if self
                    .byte(self.position + 1 + usize::from(sign))
                    .is_ascii_digit()
                {
                    self.position += 1 + usize::from(sign);
                }
            }
        }
        // Hex digits, an exponent's digits and the `n` of a BigInt.
        while self.byte(self.position).is_ascii_alphanumeric() || self.byte(self.position) == b'_' {
            self.position += 1;
        }
    }

    fn skip_digits(&mut self) {
        while self.byte(self.position).is_ascii_digit() || self.byte(self.position) == b'_' {
            self.position += 1;
        }
    }

    fn scan_string(&mut self, quote: char) -> Result<(), Unparsed> {
        self.position += 1;
        loop {
            let character = self.rest().chars().next().ok_or(Unparsed)?;
            self.position += character.len_utf8();
            if character == quote {
                return Ok(());
            }
            if character == '\\' {
                let escaped = self.rest().chars().next().ok_or(Unparsed)?;
                self.position += escaped.len_utf8();
            } else if character == '\n' || character == '\r' {
                return Err(Unparsed);
            }
        }
    }

    /// Reads template text from just after a backtick or `}`.
    fn scan_template(&mut self) -> Result<Kind, Unparsed> {
        loop {
            let rest = self.rest();
            let character = rest.chars().next().ok_or(Unparsed)?;
            if character == '`' {
                self.position += 1;
                return Ok(Kind::Template { tail: true });
            }
            if rest.starts_with("${") {
                self.position += 2;
                return Ok(Kind::Template { tail: false });
            }
            self.position += character.len_utf8();
            if character == '\\' {
                let escaped = self.rest().chars().next().ok_or(Unparsed)?;
                self.position += escaped.len_utf8();
            }
        }
    }

    /// Reads the rest of a template whose substitution ends at `close`,
    /// a `}` token.
    pub fn resume_template(&mut self, close: Token) -> Result<Token, Unparsed> {
        self.position = close.start + 1;
        let kind = self.scan_template()?;
        Ok(Token {
            kind,
            end: self.position,
            ..close
        })
    }

    /// Reads `slash`, a `/` or `/=` token, again as the regular expression
    /// literal it starts.
    pub fn regular_expression(&mut self, slash: Token) -> Result<Token, Unparsed> {
        self.position = slash.start + 1;
        let mut in_class = false;
        loop {
            let character = self.rest().chars().next().ok_or(Unparsed)?;
            self.position += character.len_utf8();
            match character {
                '\\' => {
                    let escaped = self.rest().chars().next().ok_or(Unparsed)?;
                    self.position += escaped.len_utf8();
                }
                '[' => in_class = true,
                ']' => in_class = false,
                '/' if !in_class => break,
                character if is_line_break(character) => return Err(Unparsed),
                _ => {}
            }
        }
        self.scan_word();
        Ok(Token {
            kind: Kind::String,
            end: self.position,
            ..slash
        })
    }

    /// Cuts `token`, a punctuator, down to its first character, so the
    /// rest is read as the next token.
    pub fn first_character(&mut self, token: Token) -> Token {
        self.position = token.start + 1;
        Token {
            end: self.position,
            ..token
        }
    }
}

pub(super) fn is_line_break(character: char) -> bool {
    matches!(character, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

fn is_word_character(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(character, '_' | '$' | '\\')
        || (!character.is_ascii() && !character.is_whitespace())
}
