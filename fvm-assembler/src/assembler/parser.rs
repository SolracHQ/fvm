use std::ops::Range;

use super::files::FileId;
use super::lexer::{Token, TokenKind};
use super::syntax::{LabelRef, ParsedArgument, ParsedInstruction};
use crate::error::{AssemblerError, Result};
use fvm_core::argument::Argument;
use fvm_core::opcode::Op;
use fvm_core::register::RegisterEncoding;
use fvm_core::section::Section;

// Byte offset within the section's output vec.
pub type SectionOffset = u32;

pub struct ParsedFile {
    pub rodata: Vec<ParsedArgument>,
    pub code: Vec<ParsedInstruction>,
    pub data: Vec<ParsedArgument>,
    pub labels: std::collections::HashMap<String, (Section, SectionOffset)>,
}

// Byte size of a data-section argument list up to but not including index `end`.
fn data_byte_offset(args: &[ParsedArgument], end: usize) -> SectionOffset {
    args[..end].iter().map(|a| a.size() as u32).sum()
}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    current_global: Option<String>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            current_global: None,
        }
    }

    fn peek(&self, offset: usize) -> &Token {
        let idx = (self.pos + offset).min(self.tokens.len() - 1);
        &self.tokens[idx]
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos];
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        tok
    }

    fn expect_ident(&mut self) -> Result<(String, FileId, Range<usize>)> {
        let tok = self.peek(0).clone();
        match &tok.kind {
            TokenKind::Ident(s) => {
                self.advance();
                Ok((s.clone(), tok.file, tok.span.clone()))
            }
            _ => Err(tok.parse_error("Expected identifier")),
        }
    }

    fn expect_comma(&mut self) -> Result<()> {
        let tok = self.peek(0).clone();
        match tok.kind {
            TokenKind::Comma => {
                self.advance();
                Ok(())
            }
            _ => Err(tok.parse_error("Expected ','")),
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(0).kind, TokenKind::Newline) {
            self.advance();
        }
    }

    // Consume an optional trailing newline or EOF after a statement.
    fn expect_eol(&mut self) -> Result<()> {
        let tok = self.peek(0).clone();
        match tok.kind {
            TokenKind::Newline | TokenKind::Eof => {
                self.advance();
                Ok(())
            }
            _ => Err(tok.parse_error("Expected end of line")),
        }
    }
}

// Register parsing

fn parse_register(tok: &Token) -> Option<RegisterEncoding> {
    let name = match &tok.kind {
        TokenKind::Ident(s) => s.as_str(),
        _ => return None,
    };

    if name == "sp" {
        return Some(RegisterEncoding::sp());
    }

    if name == "cr" {
        return Some(RegisterEncoding::cr());
    }

    if name == "ip" {
        return Some(RegisterEncoding::ip());
    }

    if name == "mr" {
        return Some(RegisterEncoding::mr());
    }

    // rw<n>, rh<n>, rb<n>
    let (prefix, rest): (&str, &str) = if let Some(r) = name.strip_prefix("rw") {
        ("rw", r)
    } else if let Some(r) = name.strip_prefix("rh") {
        ("rh", r)
    } else if let Some(r) = name.strip_prefix("rb") {
        ("rb", r)
    } else {
        return None;
    };

    let index: u8 = rest.parse().ok().filter(|&n: &u8| n < 16)?;
    match prefix {
        "rw" => Some(RegisterEncoding::rw(index)),
        "rh" => Some(RegisterEncoding::rh(index)),
        "rb" => Some(RegisterEncoding::rb(index)),
        _ => None,
    }
}

// Data directives

fn parse_db(
    parser: &mut Parser,
    file: FileId,
    span: Range<usize>,
) -> Result<Vec<ParsedArgument>> {
    let mut out = Vec::new();
    loop {
        let tok = parser.peek(0).clone();
        match &tok.kind {
            TokenKind::Number(n) => {
                let n = *n;
                if n > 0xFF {
                    return Err(tok.parse_error("db value out of u8 range"));
                }
                out.push(ParsedArgument::Value(Argument::Inmm8(n as u8)));
                parser.advance();
            }
            TokenKind::Char(c) => {
                out.push(ParsedArgument::Value(Argument::Inmm8(*c)));
                parser.advance();
            }
            TokenKind::String(bytes) => {
                for &b in bytes {
                    out.push(ParsedArgument::Value(Argument::Inmm8(b)));
                }
                parser.advance();
            }
            _ => break,
        }

        if matches!(parser.peek(0).kind, TokenKind::Comma) {
            parser.advance();
        } else {
            break;
        }
    }

    if out.is_empty() {
        return Err(AssemblerError::parse(file, span, "db requires at least one operand"));
    }
    Ok(out)
}

fn parse_dh(
    parser: &mut Parser,
    file: FileId,
    span: Range<usize>,
) -> Result<Vec<ParsedArgument>> {
    let mut out = Vec::new();
    loop {
        let tok = parser.peek(0).clone();
        match &tok.kind {
            TokenKind::Number(n) => {
                let n = *n;
                if n > 0xFFFF {
                    return Err(tok.parse_error("dh value out of u16 range"));
                }
                out.push(ParsedArgument::Value(Argument::Inmm16(n as u16)));
                parser.advance();
            }
            _ => break,
        }

        if matches!(parser.peek(0).kind, TokenKind::Comma) {
            parser.advance();
        } else {
            break;
        }
    }

    if out.is_empty() {
        return Err(AssemblerError::parse(file, span, "dh requires at least one operand"));
    }
    Ok(out)
}

fn parse_dw(
    parser: &mut Parser,
    file: FileId,
    span: Range<usize>,
) -> Result<Vec<ParsedArgument>> {
    let mut out = Vec::new();
    loop {
        let tok = parser.peek(0).clone();
        match &tok.kind {
            TokenKind::Number(n) => {
                out.push(ParsedArgument::Value(Argument::Inmm32(*n)));
                parser.advance();
            }
            TokenKind::Ident(name) => {
                out.push(ParsedArgument::LabelRef(LabelRef {
                    name: name.clone(),
                    file,
                    span: tok.span.clone(),
                }));
                parser.advance();
            }
            _ => break,
        }

        if matches!(parser.peek(0).kind, TokenKind::Comma) {
            parser.advance();
        } else {
            break;
        }
    }

    if out.is_empty() {
        return Err(AssemblerError::parse(file, span, "dw requires at least one operand"));
    }
    Ok(out)
}

// Instruction operand parsing

fn parse_label(parser: &mut Parser, file: FileId, span: Range<usize>) -> Result<ParsedArgument> {
    let tok = parser.peek(0).clone();
    if matches!(tok.kind, TokenKind::Dot) {
        parser.advance(); // consume '.'
        let (local, lfile, lspan) = parser.expect_ident()?;
        let global = parser.current_global.as_deref().ok_or_else(|| {
            AssemblerError::parse(
                lfile,
                lspan.clone(),
                "Local label reference without a preceding global label",
            )
        })?;
        let full_name = format!("{global}.{local}");
        Ok(ParsedArgument::LabelRef(LabelRef {
            name: full_name,
            file: lfile,
            span: lspan,
        }))
    } else if let TokenKind::Ident(_) = &tok.kind {
        let (name, file, span) = parser.expect_ident()?;
        Ok(ParsedArgument::LabelRef(LabelRef { name, file, span }))
    } else {
        Err(AssemblerError::parse(file, span, "Expected label"))
    }
}

fn parse_imm_or_label(parser: &mut Parser) -> Result<ParsedArgument> {
    let tok = parser.peek(0).clone();
    match &tok.kind {
        TokenKind::Number(n) => {
            parser.advance();
            Ok(ParsedArgument::Value(Argument::Inmm32(*n)))
        }
        TokenKind::Char(c) => {
            parser.advance();
            Ok(ParsedArgument::Value(Argument::Inmm8(*c)))
        }
        TokenKind::Dot | TokenKind::Ident(_) => parse_label(parser, tok.file, tok.span.clone()),
        _ => Err(tok.parse_error("Expected immediate or label")),
    }
}

fn parse_reg_or_imm(
    parser: &mut Parser,
    file: FileId,
    span: Range<usize>,
) -> Result<ParsedArgument> {
    let tok = parser.peek(0).clone();
    if let Some(reg) = parse_register(&tok) {
        parser.advance();
        return Ok(ParsedArgument::Value(Argument::Register(reg)));
    }
    match &tok.kind {
        TokenKind::Number(n) => {
            parser.advance();
            Ok(ParsedArgument::Value(Argument::Inmm32(*n)))
        }
        TokenKind::Char(c) => {
            parser.advance();
            Ok(ParsedArgument::Value(Argument::Inmm8(*c)))
        }
        TokenKind::Dot | TokenKind::Ident(_) => parse_label(parser, tok.file, tok.span.clone()),
        _ => Err(AssemblerError::parse(file, span, "Expected register, immediate, or label")),
    }
}

fn parse_shift_amount(parser: &mut Parser, mnemonic: &str) -> Result<ParsedArgument> {
    let tok = parser.peek(0).clone();
    if let Some(reg) = parse_register(&tok) {
        if !reg.is_rb() {
            return Err(tok.parse_error(format!("{mnemonic}: shift amount register must be rb")));
        }
        parser.advance();
        return Ok(ParsedArgument::Value(Argument::Register(reg)));
    }

    match tok.kind {
        TokenKind::Number(n) => {
            if n > 0xFF {
                return Err(tok.parse_error(format!(
                    "{mnemonic}: shift amount immediate must fit in u8"
                )));
            }
            parser.advance();
            Ok(ParsedArgument::Value(Argument::Inmm8(n as u8)))
        }
        TokenKind::Char(c) => {
            parser.advance();
            Ok(ParsedArgument::Value(Argument::Inmm8(c)))
        }
        _ => Err(tok.parse_error(format!(
            "{mnemonic}: expected rb register or imm8 shift amount"
        ))),
    }
}

fn parse_u32_or_rw(
    parser: &mut Parser,
    mnemonic: &str,
    operand_name: &str,
) -> Result<ParsedArgument> {
    let tok = parser.peek(0).clone();
    if let Some(reg) = parse_register(&tok) {
        if !reg.is_rw() {
            return Err(tok.parse_error(format!(
                "{mnemonic}: {operand_name} register must be rw"
            )));
        }
        parser.advance();
        return Ok(ParsedArgument::Value(Argument::Register(reg)));
    }

    match tok.kind {
        TokenKind::Number(n) => {
            parser.advance();
            Ok(ParsedArgument::Value(Argument::Inmm32(n)))
        }
        _ => Err(tok.parse_error(format!(
            "{mnemonic}: expected rw register or imm32 for {operand_name}"
        ))),
    }
}

// Narrow an Inmm32 to Inmm8/Inmm16 based on destination register width,
// or keep it as Inmm32. Returns an error if the value is out of range.
fn fit_imm(arg: ParsedArgument, file: FileId, span: Range<usize>, width: u8) -> Result<ParsedArgument> {
    match arg {
        ParsedArgument::Value(Argument::Inmm32(n)) => match width {
            1 => {
                if n > 0xFF {
                    return Err(AssemblerError::parse(
                        file,
                        span,
                        "Immediate out of range for 8-bit register",
                    ));
                }
                Ok(ParsedArgument::Value(Argument::Inmm8(n as u8)))
            }
            2 => {
                if n > 0xFFFF {
                    return Err(AssemblerError::parse(
                        file,
                        span,
                        "Immediate out of range for 16-bit register",
                    ));
                }
                Ok(ParsedArgument::Value(Argument::Inmm16(n as u16)))
            }
            _ => Ok(ParsedArgument::Value(Argument::Inmm32(n))),
        },
        other => Ok(other),
    }
}

// Opcode selection helpers: pick the reg-reg or reg-imm variant.

fn pick_alu(reg_op: Op, imm_op: Op, src: &ParsedArgument) -> Op {
    match src {
        ParsedArgument::Value(Argument::Register(_)) => reg_op,
        _ => imm_op,
    }
}

fn pick_jump(label_op: Op, reg_op: Op, target: &ParsedArgument) -> Op {
    match target {
        ParsedArgument::Value(Argument::Register(_)) => reg_op,
        _ => label_op,
    }
}

// Instruction parsing

fn parse_instruction(
    parser: &mut Parser,
    mnemonic: &str,
    file: FileId,
    span: Range<usize>,
) -> Result<ParsedInstruction> {
    let no_args = [
        ParsedArgument::Value(Argument::None),
        ParsedArgument::Value(Argument::None),
        ParsedArgument::Value(Argument::None),
    ];

    macro_rules! inst {
        ($op:expr) => {
            ParsedInstruction::new($op, no_args, 0)
        };
        ($op:expr, $a:expr) => {
            ParsedInstruction::new(
                $op,
                [
                    $a,
                    ParsedArgument::Value(Argument::None),
                    ParsedArgument::Value(Argument::None),
                ],
                1,
            )
        };
        ($op:expr, $a:expr, $b:expr) => {
            ParsedInstruction::new(
                $op,
                [$a, $b, ParsedArgument::Value(Argument::None)],
                2,
            )
        };
        ($op:expr, $a:expr, $b:expr, $c:expr) => {
            ParsedInstruction::new($op, [$a, $b, $c], 3)
        };
    }

    match mnemonic {
        "NOP" => Ok(inst!(Op::Nop)),
        "HALT" => Ok(inst!(Op::Halt)),
        "RET" => Ok(inst!(Op::Ret)),
        "IRET" => Ok(inst!(Op::Iret)),
        "DPL" => Ok(inst!(Op::Dpl)),

        "PUSH" => {
            let tok = parser.peek(0).clone();
            let reg = parse_register(&tok).ok_or_else(|| tok.parse_error("PUSH expects a register"))?;
            parser.advance();
            Ok(inst!(Op::Push, ParsedArgument::Value(Argument::Register(reg))))
        }

        "POP" => {
            let tok = parser.peek(0).clone();
            let reg = parse_register(&tok).ok_or_else(|| tok.parse_error("POP expects a register"))?;
            parser.advance();
            Ok(inst!(Op::Pop, ParsedArgument::Value(Argument::Register(reg))))
        }

        "NOT" => {
            let tok = parser.peek(0).clone();
            let reg = parse_register(&tok).ok_or_else(|| tok.parse_error("NOT expects a register"))?;
            parser.advance();
            Ok(inst!(Op::Not, ParsedArgument::Value(Argument::Register(reg))))
        }

        "MOV" => {
            let dst_tok = parser.peek(0).clone();
            let dst = parse_register(&dst_tok)
                .ok_or_else(|| dst_tok.parse_error("MOV: expected destination register"))?;
            if dst.is_ip() {
                return Err(dst_tok.parse_error("MOV: ip is not a valid destination; use TKR ip, rw"));
            }
            if dst.is_cr() {
                return Err(dst_tok.parse_error("MOV: cr is not a valid destination; use TKR cr, rw"));
            }
            parser.advance();
            parser.expect_comma()?;

            let src_tok = parser.peek(0).clone();
            if let Some(src_reg) = parse_register(&src_tok) {
                if src_reg.is_ip() {
                    return Err(src_tok.parse_error("MOV: ip is not a valid source; use TUR rw, ip"));
                }
                parser.advance();
                if dst.width_bytes() != src_reg.width_bytes() && !dst.is_sp() && !src_reg.is_sp() {
                    return Err(src_tok.parse_error("MOV: register views must have the same width"));
                }
                Ok(inst!(
                    Op::MovRegReg,
                    ParsedArgument::Value(Argument::Register(dst)),
                    ParsedArgument::Value(Argument::Register(src_reg))
                ))
            } else {
                let imm = parse_imm_or_label(parser)?;
                let imm = fit_imm(imm, src_tok.file, src_tok.span.clone(), dst.width_bytes())?;
                Ok(inst!(
                    Op::MovRegImm,
                    ParsedArgument::Value(Argument::Register(dst)),
                    imm
                ))
            }
        }

        "ZEXT" => {
            let dst_tok = parser.peek(0).clone();
            let dst = parse_register(&dst_tok)
                .ok_or_else(|| dst_tok.parse_error("ZEXT: expected destination register"))?;
            parser.advance();
            parser.expect_comma()?;
            let src_tok = parser.peek(0).clone();
            let src = parse_register(&src_tok)
                .ok_or_else(|| src_tok.parse_error("ZEXT: expected source register"))?;
            parser.advance();
            if dst.width_bytes() <= src.width_bytes() {
                return Err(src_tok.parse_error("ZEXT: destination must be wider than source"));
            }
            Ok(inst!(
                Op::ZeroExtend,
                ParsedArgument::Value(Argument::Register(dst)),
                ParsedArgument::Value(Argument::Register(src))
            ))
        }

        "SEXT" => {
            let dst_tok = parser.peek(0).clone();
            let dst = parse_register(&dst_tok)
                .ok_or_else(|| dst_tok.parse_error("SEXT: expected destination register"))?;
            parser.advance();
            parser.expect_comma()?;
            let src_tok = parser.peek(0).clone();
            let src = parse_register(&src_tok)
                .ok_or_else(|| src_tok.parse_error("SEXT: expected source register"))?;
            parser.advance();
            if dst.width_bytes() <= src.width_bytes() {
                return Err(src_tok.parse_error("SEXT: destination must be wider than source"));
            }
            Ok(inst!(
                Op::SignExtend,
                ParsedArgument::Value(Argument::Register(dst)),
                ParsedArgument::Value(Argument::Register(src))
            ))
        }

        op @ ("ADD" | "SUB" | "AND" | "OR" | "XOR" | "CMP" | "MUL" | "DIV" | "MOD" | "SMUL" | "SDIV" | "SMOD") => {
            let dst_tok = parser.peek(0).clone();
            let dst = parse_register(&dst_tok)
                .ok_or_else(|| dst_tok.parse_error(format!("{op}: expected destination register")))?;
            parser.advance();
            parser.expect_comma()?;

            let src_tok = parser.peek(0).clone();
            let src = parse_reg_or_imm(parser, src_tok.file, src_tok.span.clone())?;
            let src = fit_imm(src, src_tok.file, src_tok.span.clone(), dst.width_bytes())?;

            let (reg_op, imm_op) = match op {
                "ADD" => (Op::Add, Op::AddImm),
                "SUB" => (Op::Sub, Op::SubImm),
                "AND" => (Op::And, Op::AndImm),
                "OR" => (Op::Or, Op::OrImm),
                "XOR" => (Op::Xor, Op::XorImm),
                "CMP" => (Op::Cmp, Op::CmpImm),
                "MUL" => (Op::Mul, Op::MulImm),
                "DIV" => (Op::Div, Op::DivImm),
                "MOD" => (Op::Mod, Op::ModImm),
                "SMUL" => (Op::Smul, Op::SmulImm),
                "SDIV" => (Op::Sdiv, Op::SdivImm),
                "SMOD" => (Op::Smod, Op::SmodImm),
                _ => unreachable!(),
            };
            let op = pick_alu(reg_op, imm_op, &src);
            Ok(inst!(op, ParsedArgument::Value(Argument::Register(dst)), src))
        }

        op @ ("SHL" | "SHR" | "SAR" | "ROL" | "ROR") => {
            let dst_tok = parser.peek(0).clone();
            let dst = parse_register(&dst_tok)
                .ok_or_else(|| dst_tok.parse_error(format!("{op}: expected destination register")))?;
            parser.advance();
            parser.expect_comma()?;

            let src = parse_shift_amount(parser, op)?;
            let (reg_op, imm_op) = match op {
                "SHL" => (Op::ShlReg, Op::ShlImm),
                "SHR" => (Op::ShrReg, Op::ShrImm),
                "SAR" => (Op::SarReg, Op::SarImm),
                "ROL" => (Op::RolReg, Op::RolImm),
                "ROR" => (Op::RorReg, Op::RorImm),
                _ => unreachable!(),
            };
            let op = pick_alu(reg_op, imm_op, &src);
            Ok(inst!(op, ParsedArgument::Value(Argument::Register(dst)), src))
        }

        op @ ("JMP" | "JZ" | "JNZ" | "JC" | "JN" | "JO" | "JNO") => {
            let tok = parser.peek(0).clone();
            let target = parse_reg_or_imm(parser, tok.file, tok.span.clone())?;
            let (label_op, reg_op) = match op {
                "JMP" => (Op::Jmp, Op::JmpReg),
                "JZ" => (Op::Jz, Op::JzReg),
                "JNZ" => (Op::Jnz, Op::JnzReg),
                "JC" => (Op::Jc, Op::JcReg),
                "JN" => (Op::Jn, Op::JnReg),
                "JO" => (Op::Jo, Op::JoReg),
                "JNO" => (Op::Jno, Op::JnoReg),
                _ => unreachable!(),
            };
            let op = pick_jump(label_op, reg_op, &target);
            Ok(inst!(op, target))
        }

        "CALL" => {
            let tok = parser.peek(0).clone();
            let target = parse_reg_or_imm(parser, tok.file, tok.span.clone())?;
            let op = pick_jump(Op::Call, Op::CallReg, &target);
            Ok(inst!(op, target))
        }

        "IN" => {
            let dst_tok = parser.peek(0).clone();
            let dst = parse_register(&dst_tok)
                .ok_or_else(|| dst_tok.parse_error("IN: expected destination register"))?;
            parser.advance();
            parser.expect_comma()?;
            let port_tok = parser.peek(0).clone();
            let port = match &port_tok.kind {
                TokenKind::Number(n) => {
                    if *n > 0xFF {
                        return Err(port_tok.parse_error("IN: port must fit in u8"));
                    }
                    *n as u8
                }
                _ => {
                    return Err(port_tok.parse_error("IN: expected port number"));
                }
            };
            parser.advance();
            Ok(inst!(
                Op::In,
                ParsedArgument::Value(Argument::Register(dst)),
                ParsedArgument::Value(Argument::Inmm8(port))
            ))
        }

        "OUT" => {
            let port_tok = parser.peek(0).clone();
            let port = match &port_tok.kind {
                TokenKind::Number(n) => {
                    if *n > 0xFF {
                        return Err(port_tok.parse_error("OUT: port must fit in u8"));
                    }
                    *n as u8
                }
                _ => {
                    return Err(port_tok.parse_error("OUT: expected port number"));
                }
            };
            parser.advance();
            parser.expect_comma()?;
            let src_tok = parser.peek(0).clone();
            let src = parse_register(&src_tok)
                .ok_or_else(|| src_tok.parse_error("OUT: expected source register"))?;
            parser.advance();
            Ok(inst!(
                Op::Out,
                ParsedArgument::Value(Argument::Inmm8(port)),
                ParsedArgument::Value(Argument::Register(src))
            ))
        }

        "LOAD" => {
            let dst_tok = parser.peek(0).clone();
            let dst = parse_register(&dst_tok)
                .ok_or_else(|| dst_tok.parse_error("LOAD: expected destination register"))?;
            parser.advance();
            parser.expect_comma()?;
            let addr_tok = parser.peek(0).clone();
            let addr = parse_register(&addr_tok)
                .ok_or_else(|| addr_tok.parse_error("LOAD: expected address register"))?;
            parser.advance();
            if !addr.is_rw() {
                return Err(addr_tok.parse_error("LOAD: address register must be rw"));
            }
            Ok(inst!(
                Op::Load,
                ParsedArgument::Value(Argument::Register(dst)),
                ParsedArgument::Value(Argument::Register(addr))
            ))
        }

        "STORE" => {
            let addr_tok = parser.peek(0).clone();
            let addr = parse_register(&addr_tok)
                .ok_or_else(|| addr_tok.parse_error("STORE: expected address register"))?;
            parser.advance();
            if !addr.is_rw() {
                return Err(addr_tok.parse_error("STORE: address register must be rw"));
            }
            parser.expect_comma()?;
            let src_tok = parser.peek(0).clone();
            let src = parse_register(&src_tok)
                .ok_or_else(|| src_tok.parse_error("STORE: expected source register"))?;
            parser.advance();
            Ok(inst!(
                Op::Store,
                ParsedArgument::Value(Argument::Register(addr)),
                ParsedArgument::Value(Argument::Register(src))
            ))
        }

        "SIE" => {
            let idx_tok = parser.peek(0).clone();
            let idx = parse_register(&idx_tok)
                .ok_or_else(|| idx_tok.parse_error("SIE: expected index register"))?;
            if !idx.is_rb() {
                return Err(idx_tok.parse_error("SIE: index register must be rb"));
            }
            parser.advance();
            parser.expect_comma()?;
            let addr_tok = parser.peek(0).clone();
            if let Some(addr_reg) = parse_register(&addr_tok) {
                if !addr_reg.is_rw() {
                    return Err(addr_tok.parse_error("SIE: handler register must be rw"));
                }
                parser.advance();
                Ok(inst!(
                    Op::SieRegReg,
                    ParsedArgument::Value(Argument::Register(idx)),
                    ParsedArgument::Value(Argument::Register(addr_reg))
                ))
            } else {
                let imm = parse_imm_or_label(parser)?;
                Ok(inst!(
                    Op::SieRegImm,
                    ParsedArgument::Value(Argument::Register(idx)),
                    imm
                ))
            }
        }

        "INT" => {
            let tok = parser.peek(0).clone();
            if let Some(reg) = parse_register(&tok) {
                if !reg.is_rb() {
                    return Err(tok.parse_error("INT: register form requires rb"));
                }
                parser.advance();
                Ok(inst!(Op::IntReg, ParsedArgument::Value(Argument::Register(reg))))
            } else if let TokenKind::Number(n) = tok.kind {
                parser.advance();
                if n > 0xFF {
                    return Err(tok.parse_error("INT: vector index must fit in u8"));
                }
                Ok(inst!(Op::IntImm, ParsedArgument::Value(Argument::Inmm8(n as u8))))
            } else {
                Err(tok.parse_error("INT: expected vector index or register"))
            }
        }

        op @ ("TUR" | "TKR") => {
            let dst_tok = parser.peek(0).clone();
            let dst = parse_register(&dst_tok)
                .ok_or_else(|| dst_tok.parse_error(format!("{op}: expected destination register")))?;
            match op {
                "TKR" => {
                    if !dst.is_rw() && !dst.is_ip() && !dst.is_cr() && !dst.is_sp(){
                        return Err(dst_tok.parse_error("TKR: destination must be rw, ip, cr, or sp"));
                    }
                }
                _ => {
                    if !dst.is_rw() {
                        return Err(dst_tok.parse_error("TUR: destination register must be rw"));
                    }
                }
            }
            parser.advance();
            parser.expect_comma()?;

            let src_tok = parser.peek(0).clone();
            let src = parse_register(&src_tok)
                .ok_or_else(|| src_tok.parse_error(format!("{op}: expected source register")))?;
            match op {
                "TUR" => {
                    if !src.is_rw() && !src.is_ip() && !src.is_cr() {
                        return Err(src_tok.parse_error("TUR: source must be rw, ip, or cr"));
                    }
                }
                _ => {
                    if !src.is_rw() {
                        return Err(src_tok.parse_error("TKR: source register must be rw"));
                    }
                }
            }
            parser.advance();

            let opcode = match op {
                "TUR" => Op::Tur,
                "TKR" => Op::Tkr,
                _ => unreachable!(),
            };
            Ok(inst!(
                opcode,
                ParsedArgument::Value(Argument::Register(dst)),
                ParsedArgument::Value(Argument::Register(src))
            ))
        }

        "MMAP" => {
            let virt_tok = parser.peek(0).clone();
            let virt = parse_register(&virt_tok)
                .ok_or_else(|| virt_tok.parse_error("MMAP: expected virtual address register"))?;
            if !virt.is_rw() {
                return Err(virt_tok.parse_error("MMAP: virtual address register must be rw"));
            }
            parser.advance();
            parser.expect_comma()?;

            let phys_tok = parser.peek(0).clone();
            let phys = parse_register(&phys_tok)
                .ok_or_else(|| phys_tok.parse_error("MMAP: expected physical address register"))?;
            if !phys.is_rw() {
                return Err(phys_tok.parse_error("MMAP: physical address register must be rw"));
            }
            parser.advance();
            parser.expect_comma()?;

            let size = parse_u32_or_rw(parser, "MMAP", "size")?;
            let opcode = pick_alu(Op::MmapRegRegReg, Op::MmapRegRegImm, &size);
            Ok(inst!(
                opcode,
                ParsedArgument::Value(Argument::Register(virt)),
                ParsedArgument::Value(Argument::Register(phys)),
                size
            ))
        }

        "MUNMAP" => {
            let virt_tok = parser.peek(0).clone();
            let virt = parse_register(&virt_tok)
                .ok_or_else(|| virt_tok.parse_error("MUNMAP: expected virtual address register"))?;
            if !virt.is_rw() {
                return Err(virt_tok.parse_error("MUNMAP: virtual address register must be rw"));
            }
            parser.advance();
            parser.expect_comma()?;

            let size = parse_u32_or_rw(parser, "MUNMAP", "size")?;
            let opcode = pick_alu(Op::MunmapRegReg, Op::MunmapRegImm, &size);
            Ok(inst!(
                opcode,
                ParsedArgument::Value(Argument::Register(virt)),
                size
            ))
        }

        "MPROTECT" => {
            let virt_tok = parser.peek(0).clone();
            let virt = parse_register(&virt_tok)
                .ok_or_else(|| virt_tok.parse_error("MPROTECT: expected virtual page register"))?;
            if !virt.is_rw() {
                return Err(virt_tok.parse_error("MPROTECT: virtual page register must be rw"));
            }
            parser.advance();
            parser.expect_comma()?;

            // Parse page_count (rw register or imm32)
            let page_count = parse_u32_or_rw(parser, "MPROTECT", "page_count")?;
            parser.expect_comma()?;

            let perms_tok = parser.peek(0).clone();
            let perms = parse_register(&perms_tok)
                .ok_or_else(|| perms_tok.parse_error("MPROTECT: expected permissions register"))?;
            if !perms.is_rb() {
                return Err(perms_tok.parse_error("MPROTECT: permissions register must be rb"));
            }
            parser.advance();

            let opcode = match &page_count {
                ParsedArgument::Value(Argument::Register(_)) => Op::MprotectRegRegRb,
                _ => Op::MprotectRegImmRb,
            };

            Ok(inst!(
                opcode,
                ParsedArgument::Value(Argument::Register(virt)),
                page_count,
                ParsedArgument::Value(Argument::Register(perms))
            ))
        }

        _ => Err(AssemblerError::parse(file, span, format!("Unknown mnemonic: {mnemonic}"))),
    }
}

// Section directive parsing

fn parse_section_directive(parser: &mut Parser) -> Result<Section> {
    let (name, nfile, nspan) = parser.expect_ident()?;
    match name.as_str() {
        "rodata" => Ok(Section::RoData),
        "code" => Ok(Section::Code),
        "data" => Ok(Section::Data),
        _ => Err(AssemblerError::parse(nfile, nspan, format!("Unknown section: .{name}"))),
    }
}

// Dot-prefixed token handling: section directive or local label definition.

fn parse_dot_item(parser: &mut Parser) -> Result<DotItem> {
    // Peek ahead: if it's ident followed by ':', it's a local label definition.
    let is_local_label = matches!(parser.peek(0).kind, TokenKind::Ident(_))
        && matches!(parser.peek(1).kind, TokenKind::Colon);

    if is_local_label {
        let (local, lfile, lspan) = parser.expect_ident()?;
        parser.advance(); // consume ':'
        let global = parser.current_global.as_deref().ok_or_else(|| {
            AssemblerError::parse(
                lfile,
                lspan.clone(),
                "Local label without a preceding global label",
            )
        })?;
        let full_name = format!("{global}.{local}");
        return Ok(DotItem::LocalLabel(full_name));
    }

    let section = parse_section_directive(parser)?;
    Ok(DotItem::Section(section))
}

enum DotItem {
    Section(Section),
    LocalLabel(String),
}

// Top-level entry

pub fn parse(mut parser: &mut Parser) -> Result<ParsedFile> {
    let mut rodata: Vec<ParsedArgument> = Vec::new();
    let mut code: Vec<ParsedInstruction> = Vec::new();
    let mut data: Vec<ParsedArgument> = Vec::new();
    let mut labels: std::collections::HashMap<String, (Section, SectionOffset)> =
        std::collections::HashMap::new();

    let mut section = Section::Code;

    loop {
        parser.skip_newlines();

        let tok = parser.peek(0).clone();

        match &tok.kind {
            TokenKind::Eof => break,

            TokenKind::Dot => {
                parser.advance();
                let item = parse_dot_item(parser)?;
                match item {
                    DotItem::Section(s) => {
                        section = s;
                        parser.expect_eol()?;
                    }
                    DotItem::LocalLabel(full_name) => {
                        let offset = match section {
                            Section::RoData => data_byte_offset(&rodata, rodata.len()),
                            Section::Code => code.iter().map(|i| i.size as u32).sum(),
                            Section::Data => data_byte_offset(&data, data.len()),
                        };
                        if labels.contains_key(&full_name) {
                            return Err(AssemblerError::parse(
                                tok.file,
                                tok.span.clone(),
                                format!("Duplicate label: {full_name}"),
                            ));
                        }
                        labels.insert(full_name, (section, offset));
                    }
                }
            }

            // Global label: ident ':'
            TokenKind::Ident(_) if matches!(parser.peek(1).kind, TokenKind::Colon) => {
                let (name, lfile, lspan) = parser.expect_ident()?;
                parser.advance(); // consume ':'

                let offset = match section {
                    Section::RoData => data_byte_offset(&rodata, rodata.len()),
                    Section::Code => code.iter().map(|i| i.size as u32).sum(),
                    Section::Data => data_byte_offset(&data, data.len()),
                };

                if labels.contains_key(&name) {
                    return Err(AssemblerError::parse(
                        lfile,
                        lspan.clone(),
                        format!("Duplicate label: {name}"),
                    ));
                }
                labels.insert(name.clone(), (section, offset));
                parser.current_global = Some(name);
            }

            // Data directive or instruction mnemonic
            TokenKind::Ident(ident) => {
                let ident = ident.clone();
                let ifile = tok.file;
                let ispan = tok.span.clone();
                parser.advance();

                match ident.as_str() {
                    "db" | "dh" | "dw" => {
                        if matches!(section, Section::Code) {
                            return Err(AssemblerError::parse(
                                ifile,
                                ispan.clone(),
                                "Data directives are not allowed in .code",
                            ));
                        }
                        let args = match ident.as_str() {
                            "db" => parse_db(&mut parser, ifile, ispan.clone())?,
                            "dh" => parse_dh(&mut parser, ifile, ispan.clone())?,
                            "dw" => parse_dw(&mut parser, ifile, ispan.clone())?,
                            _ => unreachable!(),
                        };
                        match section {
                            Section::RoData => rodata.extend(args),
                            Section::Data => data.extend(args),
                            Section::Code => unreachable!(),
                        }
                        parser.expect_eol()?;
                    }
                    mnemonic => {
                        if !matches!(section, Section::Code) {
                            return Err(AssemblerError::parse(
                                ifile,
                                ispan.clone(),
                                "Instructions are only allowed in .code",
                            ));
                        }
                        let inst = parse_instruction(&mut parser, mnemonic, ifile, ispan.clone())?;
                        parser.expect_eol()?;
                        code.push(inst);
                    }
                }
            }

            _ => {
                return Err(tok.parse_error(format!("Unexpected token: {:?}", tok.kind)));
            }
        }
    }

    Ok(ParsedFile {
        rodata,
        code,
        data,
        labels,
    })
}
