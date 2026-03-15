//! Tokenizer for FVM assembly language.

use crate::error::{AssemblerError, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Dot,
    Colon,
    Comma,
    Equals,
    Newline,
    Number(u32),
    String(Vec<u8>),
    Char(u8),
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: u32,
    pub col: u32,
}

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
    line: u32,
    col: u32,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Lexer {
            input: input.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek(&self, offset: usize) -> char {
        self.input.get(self.pos + offset).copied().unwrap_or('\0')
    }

    fn advance(&mut self) -> char {
        let ch = self.peek(0);
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        ch
    }

    fn skip_whitespace(&mut self) {
        loop {
            match self.peek(0) {
                ' ' | '\t' | '\r' => {
                    self.advance();
                }
                '#' => {
                    // Skip comment until end of line
                    while self.peek(0) != '\n' && self.peek(0) != '\0' {
                        self.advance();
                    }
                }
                _ => break,
            }
        }
    }

    fn lex_ident(&mut self) -> String {
        let mut ident = String::new();
        while matches!(self.peek(0), 'a'..='z' | 'A'..='Z' | '_' | '0'..='9') {
            ident.push(self.advance());
        }
        ident
    }

    fn lex_hex_number(&mut self) -> Result<u32> {
        let mut num_str = String::new();
        while matches!(self.peek(0), '0'..='9' | 'a'..='f' | 'A'..='F') {
            num_str.push(self.advance());
        }
        u32::from_str_radix(&num_str, 16)
            .map_err(|_| AssemblerError::lex(self.line, self.col, "Invalid hex number"))
    }

    fn lex_oct_number(&mut self) -> Result<u32> {
        let mut num_str = String::new();
        while matches!(self.peek(0), '0'..='7') {
            num_str.push(self.advance());
        }
        u32::from_str_radix(&num_str, 8)
            .map_err(|_| AssemblerError::lex(self.line, self.col, "Invalid octal number"))
    }

    fn lex_bin_number(&mut self) -> Result<u32> {
        let mut num_str = String::new();
        while matches!(self.peek(0), '0' | '1') {
            num_str.push(self.advance());
        }
        u32::from_str_radix(&num_str, 2)
            .map_err(|_| AssemblerError::lex(self.line, self.col, "Invalid binary number"))
    }

    fn lex_dec_number(&mut self) -> Result<u32> {
        let mut num_str = String::new();
        while matches!(self.peek(0), '0'..='9') {
            num_str.push(self.advance());
        }
        num_str
            .parse::<u32>()
            .map_err(|_| AssemblerError::lex(self.line, self.col, "Invalid decimal number"))
    }

    fn lex_number(&mut self) -> Result<u32> {
        if self.peek(0) == '0' && matches!(self.peek(1), 'x' | 'X') {
            self.advance(); // '0'
            self.advance(); // 'x'
            self.lex_hex_number()
        } else if self.peek(0) == '0' && matches!(self.peek(1), 'o' | 'O') {
            self.advance(); // '0'
            self.advance(); // 'o'
            self.lex_oct_number()
        } else if self.peek(0) == '0' && matches!(self.peek(1), 'b' | 'B') {
            self.advance(); // '0'
            self.advance(); // 'b'
            self.lex_bin_number()
        } else {
            self.lex_dec_number()
        }
    }

    fn escape_char(&mut self) -> Result<u8> {
        let ch = self.advance();
        match ch {
            'n' => Ok(b'\n'),
            't' => Ok(b'\t'),
            'r' => Ok(b'\r'),
            '\\' => Ok(b'\\'),
            '"' => Ok(b'"'),
            '\'' => Ok(b'\''),
            '0' => Ok(0),
            _ => Err(AssemblerError::lex(
                self.line,
                self.col,
                format!("Invalid escape sequence: \\{}", ch),
            )),
        }
    }

    fn lex_string(&mut self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        loop {
            match self.peek(0) {
                '\0' => {
                    return Err(AssemblerError::lex(
                        self.line,
                        self.col,
                        "Unterminated string literal",
                    ));
                }
                '"' => {
                    self.advance();
                    break;
                }
                '\\' => {
                    self.advance();
                    bytes.push(self.escape_char()?);
                }
                _ch => {
                    bytes.push(self.advance() as u8);
                }
            }
        }
        Ok(bytes)
    }

    fn lex_char(&mut self) -> Result<u8> {
        let ch = match self.peek(0) {
            '\0' => {
                return Err(AssemblerError::lex(
                    self.line,
                    self.col,
                    "Unterminated character literal",
                ));
            }
            '\\' => {
                self.advance();
                self.escape_char()?
            }
            _c => self.advance() as u8,
        };

        if self.peek(0) != '\'' {
            return Err(AssemblerError::lex(
                self.line,
                self.col,
                "Expected closing single quote",
            ));
        }
        self.advance();
        Ok(ch)
    }

    fn next_token(&mut self) -> Result<Token> {
        self.skip_whitespace();
        let line = self.line;
        let col = self.col;

        match self.peek(0) {
            '\0' => Ok(Token {
                kind: TokenKind::Eof,
                line,
                col,
            }),
            '.' => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Dot,
                    line,
                    col,
                })
            }
            ':' => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Colon,
                    line,
                    col,
                })
            }
            ',' => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Comma,
                    line,
                    col,
                })
            }
            '=' => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Equals,
                    line,
                    col,
                })
            }
            '\n' => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Newline,
                    line,
                    col,
                })
            }
            '"' => {
                self.advance();
                let bytes = self.lex_string()?;
                Ok(Token {
                    kind: TokenKind::String(bytes),
                    line,
                    col,
                })
            }
            '\'' => {
                self.advance();
                let ch = self.lex_char()?;
                Ok(Token {
                    kind: TokenKind::Char(ch),
                    line,
                    col,
                })
            }
            '0'..='9' => {
                let num = self.lex_number()?;
                Ok(Token {
                    kind: TokenKind::Number(num),
                    line,
                    col,
                })
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let ident = self.lex_ident();
                Ok(Token {
                    kind: TokenKind::Ident(ident),
                    line,
                    col,
                })
            }
            ch => Err(AssemblerError::lex(
                line,
                col,
                format!("Unexpected character: {}", ch),
            )),
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = matches!(tok.kind, TokenKind::Eof);
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }
}
