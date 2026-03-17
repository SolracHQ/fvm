use std::ops::Range;

use crate::assembler::files::{FileId, FileTable};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub file: FileId,
    pub span: Range<usize>,
}

#[derive(Error, Debug)]
pub enum AssemblerError {
    #[error("{message}")]
    LexError { loc: SourceLocation, message: String },
    #[error("{message}")]
    ParseError { loc: SourceLocation, message: String },
    #[error("{message}")]
    ResolverError { loc: SourceLocation, message: String },
    #[error("{message}")]
    EmitError { loc: SourceLocation, message: String },
    #[error("IO error: {0}")]
    IoError(String),
    #[error("error: {0}")]
    FvmError(#[from] fvm_core::error::FvmError),
    #[error("{error}")]
    Context {
        #[source]
        error: Box<AssemblerError>,
        files: FileTable,
    },
}

impl AssemblerError {
    pub fn lex(file: FileId, span: Range<usize>, message: impl Into<String>) -> Self {
        AssemblerError::LexError {
            loc: SourceLocation { file, span },
            message: message.into(),
        }
    }

    pub fn parse(file: FileId, span: Range<usize>, message: impl Into<String>) -> Self {
        AssemblerError::ParseError {
            loc: SourceLocation { file, span },
            message: message.into(),
        }
    }

    pub fn resolver(file: FileId, span: Range<usize>, message: impl Into<String>) -> Self {
        AssemblerError::ResolverError {
            loc: SourceLocation { file, span },
            message: message.into(),
        }
    }

    pub fn emit(file: FileId, span: Range<usize>, message: impl Into<String>) -> Self {
        AssemblerError::EmitError {
            loc: SourceLocation { file, span },
            message: message.into(),
        }
    }

    pub fn loc(&self) -> Option<&SourceLocation> {
        match self {
            Self::LexError { loc, .. }
            | Self::ParseError { loc, .. }
            | Self::ResolverError { loc, .. }
            | Self::EmitError { loc, .. } => Some(loc),
            Self::Context { error, .. } => error.loc(),
            Self::IoError(_) | Self::FvmError(_) => None,
        }
    }

    pub fn files(&self) -> Option<&FileTable> {
        match self {
            Self::Context { files, .. } => Some(files),
            Self::LexError { .. }
            | Self::ParseError { .. }
            | Self::ResolverError { .. }
            | Self::EmitError { .. }
            | Self::IoError(_)
            | Self::FvmError(_) => None,
        }
    }

    pub fn with_files(self, files: FileTable) -> Self {
        Self::Context {
            error: Box::new(self),
            files,
        }
    }

    pub fn with_span_offset(self, offset: usize) -> Self {
        match self {
            Self::LexError { loc, message } => Self::LexError {
                loc: SourceLocation {
                    file: loc.file,
                    span: (loc.span.start + offset)..(loc.span.end + offset),
                },
                message,
            },
            Self::ParseError { loc, message } => Self::ParseError {
                loc: SourceLocation {
                    file: loc.file,
                    span: (loc.span.start + offset)..(loc.span.end + offset),
                },
                message,
            },
            Self::ResolverError { loc, message } => Self::ResolverError {
                loc: SourceLocation {
                    file: loc.file,
                    span: (loc.span.start + offset)..(loc.span.end + offset),
                },
                message,
            },
            Self::EmitError { loc, message } => Self::EmitError {
                loc: SourceLocation {
                    file: loc.file,
                    span: (loc.span.start + offset)..(loc.span.end + offset),
                },
                message,
            },
            Self::Context { error, files } => Self::Context {
                error: Box::new(error.with_span_offset(offset)),
                files,
            },
            Self::IoError(message) => Self::IoError(message),
            Self::FvmError(error) => Self::FvmError(error),
        }
    }
}

pub type Result<T> = std::result::Result<T, AssemblerError>;
