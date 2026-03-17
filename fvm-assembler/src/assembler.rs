//! Assembler: orchestrates the lexer, parser, mapper, resolver, and emitter.

pub mod diagnostic;
pub mod emitter;
pub mod files;
pub mod lexer;
pub mod parser;
pub mod preprocessor;
pub mod resolver;
pub mod syntax;

use crate::error::Result;
use fvm_core::format::FvmFormat;

pub struct AssemblyArtifacts {
    pub format: FvmFormat,
    pub files: files::FileTable,
}

impl std::ops::Deref for AssemblyArtifacts {
    type Target = FvmFormat;

    fn deref(&self) -> &Self::Target {
        &self.format
    }
}

pub fn assemble_source(source: &str) -> Result<AssemblyArtifacts> {
    let preprocessed = preprocessor::Preprocessor::preprocess_source(source)?;

    assemble_tokens(preprocessed.tokens, preprocessed.files)
}

pub fn assemble_file(path: &str) -> Result<AssemblyArtifacts> {
    let preprocessed = preprocessor::Preprocessor::preprocess_file(std::path::Path::new(path))?;

    assemble_tokens(preprocessed.tokens, preprocessed.files)
}

fn assemble_tokens(
    tokens: Vec<lexer::Token>,
    files: files::FileTable,
) -> Result<AssemblyArtifacts> {
    let mut parser = parser::Parser::new(tokens);
    let instructions = parser::parse(&mut parser).map_err(|error| error.with_files(files.clone()))?;

    let resolved = resolver::resolve(instructions).map_err(|error| error.with_files(files.clone()))?;

    let format = emitter::emit(resolved, "main").map_err(|error| error.with_files(files.clone()))?;

    Ok(AssemblyArtifacts { format, files })
}
