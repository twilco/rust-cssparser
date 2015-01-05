/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

// http://dev.w3.org/csswg/css3-syntax/#tokenization

use std::{char, num};
use std::ascii::AsciiExt;

use self::Token::*;


#[deriving(PartialEq, Show)]
pub enum Token {
    // Preserved tokens.
    Ident(String),
    AtKeyword(String),
    Hash(String),
    IDHash(String),  // Hash that is a valid ID selector.
    QuotedString(String),
    Url(String),
    Delim(char),
    Number(NumericValue),
    Percentage(NumericValue),
    Dimension(NumericValue, String),
    UnicodeRange(u32, u32),  // (start, end) of range
    WhiteSpace,
    Colon,  // :
    Semicolon,  // ;
    Comma,  // ,
    IncludeMatch, // ~=
    DashMatch, // |=
    PrefixMatch, // ^=
    SuffixMatch, // $=
    SubstringMatch, // *=
    Column, // ||
    CDO,  // <!--
    CDC,  // -->

    // Function
    Function(String),  // name

    // Simple block
    ParenthesisBlock,  // (…)
    SquareBracketBlock,  // […]
    CurlyBracketBlock,  // {…}

    // These are always invalid
    BadUrl,
    BadString,
    CloseParenthesis, // )
    CloseSquareBracket, // ]
    CloseCurlyBracket, // }
}


#[deriving(PartialEq, Show, Copy)]
pub struct NumericValue {
    pub value: f64,
    pub int_value: Option<i64>,
    // Whether the number had a `+` or `-` sign.
    pub signed: bool,
}


pub struct Tokenizer<'a> {
    input: &'a str,
    position: uint,  // All counted in bytes, not characters

    /// For `peek` and `push_back`
    buffer: Option<Token>,
}


impl<'a> Tokenizer<'a> {
    #[inline]
    pub fn new(input: &str) -> Tokenizer {
        Tokenizer {
            input: input,
            position: 0,
            buffer: None,
        }
    }

    #[inline]
    pub fn next(&mut self) -> Result<Token, ()> {
        if let Some(token) = self.buffer.take() {
            Ok(token)
        } else {
            next_token(self).ok_or(())
        }
    }

    #[inline]
    pub fn peek(&mut self) -> Result<&Token, ()> {
        match self.buffer {
            Some(ref token) => Ok(token),
            None => {
                self.buffer = next_token(self);
                self.buffer.as_ref().ok_or(())
            }
        }
    }

    #[inline]
    pub fn push_back(&mut self, token: Token) {
        assert!(self.buffer.is_none(),
                "Parser::push_back can only be called after Parser::next");
        self.buffer = Some(token);
    }

    // If false, `tokenizer.current_char()` will not panic.
    #[inline]
    fn is_eof(&self) -> bool { !self.has_at_least(0) }

    // If true, the input has at least `n` bytes left *after* the current one.
    // That is, `tokenizer.char_at(n)` will not panic.
    #[inline]
    fn has_at_least(&self, n: uint) -> bool { self.position + n < self.input.len() }

    #[inline]
    fn advance(&mut self, n: uint) { self.position += n }

    // Assumes non-EOF
    #[inline]
    fn current_char(&self) -> char { self.char_at(0) }

    #[inline]
    fn char_at(&self, offset: uint) -> char {
        self.input.char_at(self.position + offset)
    }

    #[inline]
    fn has_newline_at(&self, offset: uint) -> bool {
        self.position + offset < self.input.len() &&
        matches!(self.char_at(offset), '\n' | '\r' | '\x0C')
    }

    #[inline]
    fn consume_char(&mut self) -> char {
        let range = self.input.char_range_at(self.position);
        self.position = range.next;
        range.ch
    }

    #[inline]
    fn starts_with(&self, needle: &str) -> bool {
        self.input.slice_from(self.position).starts_with(needle)
    }

    #[inline]
    fn slice_from(&self, start_pos: uint) -> &str {
        self.input.slice(start_pos, self.position)
    }
}


fn next_token(tokenizer: &mut Tokenizer) -> Option<Token> {
    consume_comments(tokenizer);
    if tokenizer.is_eof() {
        return None
    }
    let c = tokenizer.current_char();
    let token = match c {
        '\t' | '\n' | ' ' | '\r' | '\x0C' => {
            while !tokenizer.is_eof() {
                match tokenizer.current_char() {
                    ' ' | '\t' | '\n' | '\r' | '\x0C' => tokenizer.advance(1),
                    _ => break,
                }
            }
            WhiteSpace
        },
        '"' => consume_string(tokenizer, false),
        '#' => {
            tokenizer.advance(1);
            if is_ident_start(tokenizer) { IDHash(consume_name(tokenizer)) }
            else if !tokenizer.is_eof() && match tokenizer.current_char() {
                'a'...'z' | 'A'...'Z' | '0'...'9' | '-' | '_' => true,
                '\\' => !tokenizer.has_newline_at(1),
                _ => c > '\x7F',  // Non-ASCII
            } { Hash(consume_name(tokenizer)) }
            else { Delim(c) }
        },
        '$' => {
            if tokenizer.starts_with("$=") { tokenizer.advance(2); SuffixMatch }
            else { tokenizer.advance(1); Delim(c) }
        },
        '\'' => consume_string(tokenizer, true),
        '(' => { tokenizer.advance(1); ParenthesisBlock },
        ')' => { tokenizer.advance(1); CloseParenthesis },
        '*' => {
            if tokenizer.starts_with("*=") { tokenizer.advance(2); SubstringMatch }
            else { tokenizer.advance(1); Delim(c) }
        },
        '+' => {
            if (
                tokenizer.has_at_least(1)
                && matches!(tokenizer.char_at(1), '0'...'9')
            ) || (
                tokenizer.has_at_least(2)
                && tokenizer.char_at(1) == '.'
                && matches!(tokenizer.char_at(2), '0'...'9')
            ) {
                consume_numeric(tokenizer)
            } else {
                tokenizer.advance(1);
                Delim(c)
            }
        },
        ',' => { tokenizer.advance(1); Comma },
        '-' => {
            if (
                tokenizer.has_at_least(1)
                && matches!(tokenizer.char_at(1), '0'...'9')
            ) || (
                tokenizer.has_at_least(2)
                && tokenizer.char_at(1) == '.'
                && matches!(tokenizer.char_at(2), '0'...'9')
            ) {
                consume_numeric(tokenizer)
            } else if tokenizer.starts_with("-->") {
                tokenizer.advance(3);
                CDC
            } else if is_ident_start(tokenizer) {
                consume_ident_like(tokenizer)
            } else {
                tokenizer.advance(1);
                Delim(c)
            }
        },
        '.' => {
            if tokenizer.has_at_least(1)
                && matches!(tokenizer.char_at(1), '0'...'9'
            ) {
                consume_numeric(tokenizer)
            } else {
                tokenizer.advance(1);
                Delim(c)
            }
        }
        '0'...'9' => consume_numeric(tokenizer),
        ':' => { tokenizer.advance(1); Colon },
        ';' => { tokenizer.advance(1); Semicolon },
        '<' => {
            if tokenizer.starts_with("<!--") {
                tokenizer.advance(4);
                CDO
            } else {
                tokenizer.advance(1);
                Delim(c)
            }
        },
        '@' => {
            tokenizer.advance(1);
            if is_ident_start(tokenizer) { AtKeyword(consume_name(tokenizer)) }
            else { Delim(c) }
        },
        'u' | 'U' => {
            if tokenizer.has_at_least(2)
               && tokenizer.char_at(1) == '+'
               && matches!(tokenizer.char_at(2), '0'...'9' | 'a'...'f' | 'A'...'F' | '?')
            { consume_unicode_range(tokenizer) }
            else { consume_ident_like(tokenizer) }
        },
        'a'...'z' | 'A'...'Z' | '_' | '\0' => consume_ident_like(tokenizer),
        '[' => { tokenizer.advance(1); SquareBracketBlock },
        '\\' => {
            if !tokenizer.has_newline_at(1) { consume_ident_like(tokenizer) }
            else { tokenizer.advance(1); Delim(c) }
        },
        ']' => { tokenizer.advance(1); CloseSquareBracket },
        '^' => {
            if tokenizer.starts_with("^=") { tokenizer.advance(2); PrefixMatch }
            else { tokenizer.advance(1); Delim(c) }
        },
        '{' => { tokenizer.advance(1); CurlyBracketBlock },
        '|' => {
            if tokenizer.starts_with("|=") { tokenizer.advance(2); DashMatch }
            else if tokenizer.starts_with("||") { tokenizer.advance(2); Column }
            else { tokenizer.advance(1); Delim(c) }
        },
        '}' => { tokenizer.advance(1); CloseCurlyBracket },
        '~' => {
            if tokenizer.starts_with("~=") { tokenizer.advance(2); IncludeMatch }
            else { tokenizer.advance(1); Delim(c) }
        },
        _ => {
            if c > '\x7F' {  // Non-ASCII
                consume_ident_like(tokenizer)
            } else {
                tokenizer.advance(1);
                Delim(c)
            }
        },
    };
    Some(token)
}


#[inline]
fn consume_comments(tokenizer: &mut Tokenizer) {
    while tokenizer.starts_with("/*") {
        tokenizer.advance(2);  // +2 to consume "/*"
        while !tokenizer.is_eof() {
            if tokenizer.consume_char() == '*' &&
               !tokenizer.is_eof() &&
               tokenizer.current_char() == '/' {
                tokenizer.advance(1);
                break
            }
        }
    }
}


fn consume_string(tokenizer: &mut Tokenizer, single_quote: bool) -> Token {
    match consume_quoted_string(tokenizer, single_quote) {
        Ok(value) => QuotedString(value),
        Err(()) => BadString
    }
}


/// Return `Err(())` on syntax error (ie. unescaped newline)
fn consume_quoted_string(tokenizer: &mut Tokenizer, single_quote: bool) -> Result<String, ()> {
    tokenizer.advance(1);  // Skip the initial quote
    let mut string = String::new();
    while !tokenizer.is_eof() {
        if matches!(tokenizer.current_char(), '\n' | '\r' | '\x0C') {
            return Err(());
        }
        match tokenizer.consume_char() {
            '"' if !single_quote => break,
            '\'' if single_quote => break,
            '\\' => {
                if !tokenizer.is_eof() {
                    match tokenizer.current_char() {
                        // Escaped newline
                        '\n' | '\x0C' => tokenizer.advance(1),
                        '\r' => {
                            tokenizer.advance(1);
                            if !tokenizer.is_eof() && tokenizer.current_char() == '\n' {
                                tokenizer.advance(1);
                            }
                        }
                        _ => string.push(consume_escape(tokenizer))
                    }
                }
                // else: escaped EOF, do nothing.
            }
            '\0' => string.push('\u{FFFD}'),
            c => string.push(c),
        }
    }
    Ok(string)
}


#[inline]
fn is_ident_start(tokenizer: &mut Tokenizer) -> bool {
    !tokenizer.is_eof() && match tokenizer.current_char() {
        'a'...'z' | 'A'...'Z' | '_' | '\0' => true,
        '-' => tokenizer.has_at_least(1) && match tokenizer.char_at(1) {
            'a'...'z' | 'A'...'Z' | '-' | '_' | '\0' => true,
            '\\' => !tokenizer.has_newline_at(1),
            c => c > '\x7F',  // Non-ASCII
        },
        '\\' => !tokenizer.has_newline_at(1),
        c => c > '\x7F',  // Non-ASCII
    }
}


fn consume_ident_like(tokenizer: &mut Tokenizer) -> Token {
    let value = consume_name(tokenizer);
    if !tokenizer.is_eof() && tokenizer.current_char() == '(' {
        tokenizer.advance(1);
        if value.eq_ignore_ascii_case("url") { consume_url(tokenizer) }
        else { Function(value) }
    } else {
        Ident(value)
    }
}

fn consume_name(tokenizer: &mut Tokenizer) -> String {
    let mut value = String::new();
    while !tokenizer.is_eof() {
        let c = tokenizer.current_char();
        value.push(match c {
            'a'...'z' | 'A'...'Z' | '0'...'9' | '_' | '-'  => { tokenizer.advance(1); c },
            '\\' => {
                if tokenizer.has_newline_at(1) { break }
                tokenizer.advance(1);
                consume_escape(tokenizer)
            },
            '\0' => { tokenizer.advance(1); '\u{FFFD}' },
            _ => if c > '\x7F' { tokenizer.consume_char() }  // Non-ASCII
                 else { break }
        })
    }
    value
}


fn consume_digits(tokenizer: &mut Tokenizer) {
    while !tokenizer.is_eof() {
        match tokenizer.current_char() {
            '0'...'9' => tokenizer.advance(1),
            _ => break
        }
    }
}


fn consume_numeric(tokenizer: &mut Tokenizer) -> Token {
    // Parse [+-]?\d*(\.\d+)?([eE][+-]?\d+)?
    // But this is always called so that there is at least one digit in \d*(\.\d+)?
    let start_pos = tokenizer.position;
    let mut is_integer = true;
    let signed = matches!(tokenizer.current_char(), '-' | '+');
    if signed {
        tokenizer.advance(1);
    }
    consume_digits(tokenizer);
    if tokenizer.has_at_least(1) && tokenizer.current_char() == '.'
            && matches!(tokenizer.char_at(1), '0'...'9') {
        is_integer = false;
        tokenizer.advance(2);  // '.' and first digit
        consume_digits(tokenizer);
    }
    if (
        tokenizer.has_at_least(1)
        && matches!(tokenizer.current_char(), 'e' | 'E')
        && matches!(tokenizer.char_at(1), '0'...'9')
    ) || (
        tokenizer.has_at_least(2)
        && matches!(tokenizer.current_char(), 'e' | 'E')
        && matches!(tokenizer.char_at(1), '+' | '-')
        && matches!(tokenizer.char_at(2), '0'...'9')
    ) {
        is_integer = false;
        tokenizer.advance(2);  // 'e' or 'E', and sign or first digit
        consume_digits(tokenizer);
    }
    let (value, int_value) = {
        let mut repr = tokenizer.slice_from(start_pos);
        // Remove any + sign as int::from_str() does not parse them.
        if repr.starts_with("+") {
            repr = repr.slice_from(1)
        }
        // TODO: handle overflow
        (from_str::<f64>(repr).unwrap(), if is_integer {
            Some(from_str::<i64>(repr).unwrap())
        } else {
            None
        })
    };
    let value = NumericValue {
        value: value,
        int_value: int_value,
        signed: signed,
    };
    if !tokenizer.is_eof() && tokenizer.current_char() == '%' {
        tokenizer.advance(1);
        Percentage(value)
    }
    else if is_ident_start(tokenizer) { Dimension(value, consume_name(tokenizer)) }
    else { Number(value) }
}


fn consume_url(tokenizer: &mut Tokenizer) -> Token {
    while !tokenizer.is_eof() {
        match tokenizer.current_char() {
            ' ' | '\t' | '\n' | '\r' | '\x0C' => tokenizer.advance(1),
            '"' => return consume_quoted_url(tokenizer, false),
            '\'' => return consume_quoted_url(tokenizer, true),
            ')' => { tokenizer.advance(1); break },
            _ => return consume_unquoted_url(tokenizer),
        }
    }
    return Url(String::new());

    fn consume_quoted_url(tokenizer: &mut Tokenizer, single_quote: bool) -> Token {
        match consume_quoted_string(tokenizer, single_quote) {
            Ok(value) => consume_url_end(tokenizer, value),
            Err(()) => consume_bad_url(tokenizer),
        }
    }

    fn consume_unquoted_url(tokenizer: &mut Tokenizer) -> Token {
        let mut string = String::new();
        while !tokenizer.is_eof() {
            let next_char = match tokenizer.consume_char() {
                ' ' | '\t' | '\n' | '\r' | '\x0C' => return consume_url_end(tokenizer, string),
                ')' => break,
                '\x01'...'\x08' | '\x0B' | '\x0E'...'\x1F' | '\x7F'  // non-printable
                    | '"' | '\'' | '(' => return consume_bad_url(tokenizer),
                '\\' => {
                    if tokenizer.has_newline_at(0) {
                        return consume_bad_url(tokenizer)
                    }
                    consume_escape(tokenizer)
                },
                '\0' => '\u{FFFD}',
                c => c
            };
            string.push(next_char)
        }
        Url(string)
    }

    fn consume_url_end(tokenizer: &mut Tokenizer, string: String) -> Token {
        while !tokenizer.is_eof() {
            match tokenizer.consume_char() {
                ' ' | '\t' | '\n' | '\r' | '\x0C' => (),
                ')' => break,
                _ => return consume_bad_url(tokenizer)
            }
        }
        Url(string)
    }

    fn consume_bad_url(tokenizer: &mut Tokenizer) -> Token {
        // Consume up to the closing )
        while !tokenizer.is_eof() {
            match tokenizer.consume_char() {
                ')' => break,
                '\\' => tokenizer.advance(1), // Skip an escaped ')' or '\'
                _ => ()
            }
        }
        BadUrl
    }
}



fn consume_unicode_range(tokenizer: &mut Tokenizer) -> Token {
    tokenizer.advance(2);  // Skip U+
    let mut hex = String::new();
    while hex.len() < 6 && !tokenizer.is_eof()
          && matches!(tokenizer.current_char(), '0'...'9' | 'A'...'F' | 'a'...'f') {
        hex.push(tokenizer.consume_char());
    }
    let max_question_marks = 6u - hex.len();
    let mut question_marks = 0u;
    while question_marks < max_question_marks && !tokenizer.is_eof()
            && tokenizer.current_char() == '?' {
        question_marks += 1;
        tokenizer.advance(1)
    }
    let first: u32 = if hex.len() > 0 {
        num::from_str_radix(hex.as_slice(), 16).unwrap()
    } else { 0 };
    let start;
    let end;
    if question_marks > 0 {
        start = first << (question_marks * 4);
        end = ((first + 1) << (question_marks * 4)) - 1;
    } else {
        start = first;
        hex.truncate(0);
        if !tokenizer.is_eof() && tokenizer.current_char() == '-' {
            tokenizer.advance(1);
            while hex.len() < 6 && !tokenizer.is_eof() {
                let c = tokenizer.current_char();
                match c {
                    '0'...'9' | 'A'...'F' | 'a'...'f' => {
                        hex.push(c); tokenizer.advance(1) },
                    _ => break
                }
            }
        }
        end = if hex.len() > 0 { num::from_str_radix(hex.as_slice(), 16).unwrap() } else { start }
    }
    UnicodeRange(start, end)
}


// Assumes that the U+005C REVERSE SOLIDUS (\) has already been consumed
// and that the next input character has already been verified
// to not be a newline.
fn consume_escape(tokenizer: &mut Tokenizer) -> char {
    if tokenizer.is_eof() { return '\u{FFFD}' }  // Escaped EOF
    let c = tokenizer.consume_char();
    match c {
        '0'...'9' | 'A'...'F' | 'a'...'f' => {
            let mut hex = String::from_char(1, c);
            while hex.len() < 6 && !tokenizer.is_eof() {
                let c = tokenizer.current_char();
                match c {
                    '0'...'9' | 'A'...'F' | 'a'...'f' => {
                        hex.push(c); tokenizer.advance(1) },
                    _ => break
                }
            }
            if !tokenizer.is_eof() {
                match tokenizer.current_char() {
                    ' ' | '\t' | '\n' | '\x0C' => tokenizer.advance(1),
                    '\r' => {
                        tokenizer.advance(1);
                        if !tokenizer.is_eof() && tokenizer.current_char() == '\n' {
                            tokenizer.advance(1);
                        }
                    }
                    _ => ()
                }
            }
            static REPLACEMENT_CHAR: char = '\u{FFFD}';
            let c: u32 = num::from_str_radix(hex.as_slice(), 16).unwrap();
            if c != 0 {
                let c = char::from_u32(c);
                c.unwrap_or(REPLACEMENT_CHAR)
            } else {
                REPLACEMENT_CHAR
            }
        },
        c => c
    }
}
