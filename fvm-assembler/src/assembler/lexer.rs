//! Tokenizer for FVM assembly language.

use std::ops::Range;

use logos::Logos;

use super::files::FileId;
use crate::error::{AssemblerError, Result};

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(error = ())]
#[logos(skip r"[ \t\r]+")]
pub enum TokenKind {
    #[regex(r"#", skip_comment)]
    Comment,
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Ident(String),
    #[token(".")]
    Dot,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token("=")]
    Equals,
    #[token("\n")]
    Newline,
    #[regex(r"0[xX][0-9a-fA-F]+|0[oO][0-7]+|0[bB][01]+|[0-9]+", lex_number)]
    Number(u32),
    #[token("\"", lex_string)]
    String(Vec<u8>),
    #[token("'", lex_char)]
    Char(u8),
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Range<usize>,
    pub file: FileId,
    #[allow(dead_code)]
    pub line: u32,
    #[allow(dead_code)]
    pub col: u32,
}

impl Token {
    pub fn parse_error(&self, message: impl Into<String>) -> AssemblerError {
        AssemblerError::parse(self.file, self.span.clone(), message)
    }

    #[allow(dead_code)]
    pub fn lex_error(&self, message: impl Into<String>) -> AssemblerError {
        AssemblerError::lex(self.file, self.span.clone(), message)
    }
}

pub(crate) fn make_token(kind: TokenKind, file: FileId, source: &str, span: Range<usize>) -> Token {
    let (line, col) = line_col(source, span.start);
    Token {
        kind,
        span,
        file,
        line,
        col,
    }
}

fn lex_number(lex: &mut logos::Lexer<TokenKind>) -> std::result::Result<u32, ()> {
    let s = lex.slice();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).map_err(|_| ())
    } else if let Some(oct) = s.strip_prefix("0o").or_else(|| s.strip_prefix("0O")) {
        u32::from_str_radix(oct, 8).map_err(|_| ())
    } else if let Some(bin) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        u32::from_str_radix(bin, 2).map_err(|_| ())
    } else {
        s.parse::<u32>().map_err(|_| ())
    }
}

fn skip_comment(lex: &mut logos::Lexer<TokenKind>) -> logos::Skip {
    let mut bump = 0usize;

    for ch in lex.remainder().chars() {
        if ch == '\n' {
            break;
        }
        bump += ch.len_utf8();
    }

    lex.bump(bump);
    logos::Skip
}

fn escape_byte(ch: char) -> Option<u8> {
    match ch {
        'n' => Some(b'\n'),
        't' => Some(b'\t'),
        'r' => Some(b'\r'),
        '\\' => Some(b'\\'),
        '"' => Some(b'"'),
        '\'' => Some(b'\''),
        '0' => Some(0),
        _ => None,
    }
}

fn lex_string(lex: &mut logos::Lexer<TokenKind>) -> std::result::Result<Vec<u8>, ()> {
    let remainder = lex.remainder();
    let mut bytes = Vec::new();
    let mut chars = remainder.char_indices();

    loop {
        match chars.next() {
            None => return Err(()),
            Some((index, '"')) => {
                lex.bump(index + 1);
                return Ok(bytes);
            }
            Some((_, '\\')) => {
                let (_, escaped) = chars.next().ok_or(())?;
                bytes.push(escape_byte(escaped).ok_or(())?);
            }
            Some((_, ch)) => {
                let mut buf = [0u8; 4];
                bytes.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            }
        }
    }
}

fn lex_char(lex: &mut logos::Lexer<TokenKind>) -> std::result::Result<u8, ()> {
    let remainder = lex.remainder();
    let mut chars = remainder.char_indices();

    let (_, first) = chars.next().ok_or(())?;
    let byte = if first == '\\' {
        let (_, escaped) = chars.next().ok_or(())?;
        escape_byte(escaped).ok_or(())?
    } else {
        first as u8
    };

    let (close_index, close_char) = chars.next().ok_or(())?;
    if close_char != '\'' {
        return Err(());
    }

    lex.bump(close_index + 1);
    Ok(byte)
}

fn line_col(source: &str, offset: usize) -> (u32, u32) {
    let mut line = 1u32;
    let mut col = 1u32;

    for ch in source[..offset.min(source.len())].chars() {
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }

    (line, col)
}

pub fn tokenize(source: &str, file: FileId) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut lexer = TokenKind::lexer(source);

    loop {
        match lexer.next() {
            None => {
                let span = lexer.span();
                tokens.push(make_token(TokenKind::Eof, file, source, span));
                break;
            }
            Some(Ok(kind)) => {
                let span = lexer.span();
                tokens.push(make_token(kind, file, source, span));
            }
            Some(Err(())) => {
                let span = lexer.span();
                return Err(AssemblerError::lex(
                    file,
                    span,
                    format!("Unexpected character: {:?}", lexer.slice()),
                ));
            }
        }
    }

    Ok(tokens)
}
