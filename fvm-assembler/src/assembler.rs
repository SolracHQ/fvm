//! Assembler: orchestrates the lexer, parser, mapper, resolver, and emitter.

pub mod emitter;
pub mod lexer;
pub mod parser;
pub mod resolver;

use crate::error::Result;

use fvm_core::format::FvmFormat;

pub fn assemble_source(source: &str) -> Result<FvmFormat> {
    // Lex
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize()?;

    // Parse
    let mut parser = parser::Parser::new(tokens);
    let instructions = parser::parse(&mut parser)?;

    // Resolve
    let resolved = resolver::resolve(instructions)?;

    // Emit
    let entry_point = "main";
    emitter::emit(resolved, entry_point)
}

pub fn assemble_file(path: &str) -> Result<FvmFormat> {
    let source = std::fs::read_to_string(path).map_err(|e| {
        crate::error::AssemblerError::IoError(format!("Failed to read file {}: {}", path, e))
    })?;
    assemble_source(&source)
}
