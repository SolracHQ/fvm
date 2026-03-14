use thiserror::Error;

#[derive(Error, Debug)]
pub enum AssemblerError {
    #[error("Lexer error at {line}:{col}: {message}")]
    LexError {
        line: u32,
        col: u32,
        message: String,
    },
    #[error("Parse error at {line}:{col}: {message}")]
    ParseError {
        line: u32,
        col: u32,
        message: String,
    },
    #[error("Resolver error at {line}:{col}: {message}")]
    ResolverError {
        line: u32,
        col: u32,
        message: String,
    },
    #[error("Emit error at {line}:{col}: {message}")]
    EmitError {
        line: u32,
        col: u32,
        message: String,
    },
    #[error("IO error: {0}")]
    IoError(String),
    #[error("error: {0}")]
    FvmError(#[from] fvm_core::error::FvmError),
}

impl AssemblerError {
    pub fn lex(line: u32, col: u32, message: impl Into<String>) -> Self {
        AssemblerError::LexError {
            line,
            col,
            message: message.into(),
        }
    }

    pub fn parse(line: u32, col: u32, message: impl Into<String>) -> Self {
        AssemblerError::ParseError {
            line,
            col,
            message: message.into(),
        }
    }

    pub fn resolver(line: u32, col: u32, message: impl Into<String>) -> Self {
        AssemblerError::ResolverError {
            line,
            col,
            message: message.into(),
        }
    }

    pub fn emit(line: u32, col: u32, message: impl Into<String>) -> Self {
        AssemblerError::EmitError {
            line,
            col,
            message: message.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, AssemblerError>;
