//! FVM Assembler Library
//!
//! This library provides the assembler components for the FVM (Fast Virtual Machine).

pub mod assembler;
pub mod error;

pub use assembler::{assemble_file, assemble_source, AssemblyArtifacts};
pub use error::{AssemblerError, Result};
