//! Tests for the lexer component

#[cfg(test)]
mod tests {
    use fvm_assembler::assembler::lexer::{tokenize, Token, TokenKind};

    fn tokenize_source(source: &str) -> Vec<Token> {
        tokenize(source, 0).unwrap()
    }

    #[test]
    fn test_tokenize_empty_string() {
        let tokens = tokenize_source("");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].kind, TokenKind::Eof));
    }

    #[test]
    fn test_tokenize_single_ident() {
        let tokens = tokenize_source("NOP");
        assert_eq!(tokens.len(), 2);
        match &tokens[0].kind {
            TokenKind::Ident(s) => assert_eq!(s, "NOP"),
            _ => panic!("Expected ident"),
        }
    }

    #[test]
    fn test_tokenize_number() {
        let tokens = tokenize_source("42");
        assert_eq!(tokens.len(), 2);
        match tokens[0].kind {
            TokenKind::Number(n) => assert_eq!(n, 42),
            _ => panic!("Expected number"),
        }
    }

    #[test]
    fn test_tokenize_hex_number() {
        let tokens = tokenize_source("0xFF00");
        assert_eq!(tokens.len(), 2);
        match tokens[0].kind {
            TokenKind::Number(n) => assert_eq!(n, 0xFF00),
            _ => panic!("Expected hex number"),
        }
    }

    #[test]
    fn test_tokenize_string() {
        let tokens = tokenize_source("\"Hello\"");
        assert_eq!(tokens.len(), 2);
        match &tokens[0].kind {
            TokenKind::String(bytes) => {
                assert_eq!(bytes, b"Hello");
            }
            _ => panic!("Expected string"),
        }
    }

    #[test]
    fn test_tokenize_char() {
        let tokens = tokenize_source("'A'");
        assert_eq!(tokens.len(), 2);
        match tokens[0].kind {
            TokenKind::Char(c) => assert_eq!(c, b'A'),
            _ => panic!("Expected char"),
        }
    }

    #[test]
    fn test_tokenize_dot() {
        let tokens = tokenize_source(".");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0].kind, TokenKind::Dot));
    }

    #[test]
    fn test_tokenize_colon() {
        let tokens = tokenize_source(":");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0].kind, TokenKind::Colon));
    }

    #[test]
    fn test_tokenize_comma() {
        let tokens = tokenize_source(",");
        assert_eq!(tokens.len(), 2);
        assert!(matches!(tokens[0].kind, TokenKind::Comma));
    }

    #[test]
    fn test_tokenize_newline() {
        let tokens = tokenize_source("NOP\nHALT");
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Newline)));
    }

    #[test]
    fn test_tokenize_comment_ignored() {
        let tokens = tokenize_source("NOP # This is a comment");
        // Should only have NOP + EOF, comment is ignored
        assert_eq!(tokens.len(), 2);
        match &tokens[0].kind {
            TokenKind::Ident(s) => assert_eq!(s, "NOP"),
            _ => panic!("Expected NOP"),
        }
    }

    #[test]
    fn test_tokenize_section_directive() {
        let tokens = tokenize_source(".code");
        assert!(matches!(tokens[0].kind, TokenKind::Dot));
        match &tokens[1].kind {
            TokenKind::Ident(s) => assert_eq!(s, "code"),
            _ => panic!("Expected 'code'"),
        }
    }

    #[test]
    fn test_tokenize_register_rw() {
        let tokens = tokenize_source("rw0");
        match &tokens[0].kind {
            TokenKind::Ident(s) => assert_eq!(s, "rw0"),
            _ => panic!("Expected register identifier"),
        }
    }

    #[test]
    fn test_tokenize_register_rh() {
        let tokens = tokenize_source("rh0");
        match &tokens[0].kind {
            TokenKind::Ident(s) => assert_eq!(s, "rh0"),
            _ => panic!("Expected register identifier"),
        }
    }

    #[test]
    fn test_tokenize_register_rb() {
        let tokens = tokenize_source("rb0");
        match &tokens[0].kind {
            TokenKind::Ident(s) => assert_eq!(s, "rb0"),
            _ => panic!("Expected register identifier"),
        }
    }

    #[test]
    fn test_tokenize_line_col_tracking() {
        let tokens = tokenize_source("NOP\nHALT");
        assert_eq!(tokens[0].line, 1);
        assert_eq!(tokens[0].col, 1);
        assert_eq!(tokens[0].file, 0);
        assert_eq!(tokens[0].span, 0..3);
        // After newline
        let halt_token = tokens
            .iter()
            .find(|t| matches!(&t.kind, TokenKind::Ident(s) if s == "HALT"))
            .unwrap();
        assert_eq!(halt_token.line, 2);
        assert_eq!(halt_token.file, 0);
    }

    #[test]
    fn test_tokenize_complex_instruction() {
        let tokens = tokenize_source("MOV rw0, 42");
        assert_eq!(tokens.len(), 5); // MOV + ident + comma + number + EOF
        match &tokens[0].kind {
            TokenKind::Ident(s) => assert_eq!(s, "MOV"),
            _ => panic!("Expected MOV"),
        }
        match &tokens[1].kind {
            TokenKind::Ident(s) => assert_eq!(s, "rw0"),
            _ => panic!("Expected rw0"),
        }
        assert!(matches!(tokens[2].kind, TokenKind::Comma));
    }

    #[test]
    fn test_tokenize_invalid_char_literal() {
        let result = tokenize("'ABC'", 0);
        assert!(result.is_err(), "Multi-char literal should be error");
    }
}
