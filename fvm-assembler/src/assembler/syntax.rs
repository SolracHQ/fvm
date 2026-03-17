use std::ops::Range;

use crate::assembler::files::FileId;
use crate::error::SourceLocation;
use fvm_core::argument::Argument;
use fvm_core::opcode::Op;

#[derive(Debug, Clone)]
pub struct LabelRef {
    pub name: String,
    pub file: FileId,
    pub span: Range<usize>,
}

impl LabelRef {
    pub fn loc(&self) -> SourceLocation {
        SourceLocation {
            file: self.file,
            span: self.span.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ParsedArgument {
    Value(Argument),
    LabelRef(LabelRef),
}

impl ParsedArgument {
    pub fn size(&self) -> usize {
        match self {
            Self::Value(argument) => argument.size(),
            Self::LabelRef(_) => 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedInstruction {
    pub opcode: Op,
    pub arguments: [ParsedArgument; 3],
    pub argument_count: usize,
    pub size: usize,
}

impl ParsedInstruction {
    pub fn new(opcode: Op, arguments: [ParsedArgument; 3], argument_count: usize) -> Self {
        let size = 1
            + arguments
                .iter()
                .take(argument_count)
                .map(ParsedArgument::size)
                .sum::<usize>();

        Self {
            opcode,
            arguments,
            argument_count,
            size,
        }
    }
}