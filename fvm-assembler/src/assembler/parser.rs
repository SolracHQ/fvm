use super::lexer::{Token, TokenKind};
use crate::error::{AssemblerError, Result};
use fvm_core::argument::Argument;
use fvm_core::instruction::Instruction;
use fvm_core::opcode::Op;
use fvm_core::register::RegisterEncoding;
use fvm_core::section::Section;

// Byte offset within the section's output vec.
pub type SectionOffset = u32;

pub struct ParsedFile {
    pub rodata: Vec<Argument>,
    pub code: Vec<Instruction>,
    pub data: Vec<Argument>,
    pub labels: std::collections::HashMap<String, (Section, SectionOffset)>,
}

// Byte size of a data-section argument list up to but not including index `end`.
fn data_byte_offset(args: &[Argument], end: usize) -> SectionOffset {
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

    fn expect_ident(&mut self) -> Result<(String, u32, u32)> {
        let tok = self.peek(0).clone();
        match &tok.kind {
            TokenKind::Ident(s) => {
                self.advance();
                Ok((s.clone(), tok.line, tok.col))
            }
            _ => Err(AssemblerError::parse(
                tok.line,
                tok.col,
                "Expected identifier",
            )),
        }
    }

    fn expect_comma(&mut self) -> Result<()> {
        let tok = self.peek(0).clone();
        match tok.kind {
            TokenKind::Comma => {
                self.advance();
                Ok(())
            }
            _ => Err(AssemblerError::parse(tok.line, tok.col, "Expected ','")),
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
            _ => Err(AssemblerError::parse(
                tok.line,
                tok.col,
                "Expected end of line",
            )),
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

fn parse_db(parser: &mut Parser, line: u32, col: u32) -> Result<Vec<Argument>> {
    let mut out = Vec::new();
    loop {
        let tok = parser.peek(0).clone();
        match &tok.kind {
            TokenKind::Number(n) => {
                let n = *n;
                if n > 0xFF {
                    return Err(AssemblerError::parse(
                        tok.line,
                        tok.col,
                        "db value out of u8 range",
                    ));
                }
                out.push(Argument::Inmm8(n as u8));
                parser.advance();
            }
            TokenKind::Char(c) => {
                out.push(Argument::Inmm8(*c));
                parser.advance();
            }
            TokenKind::String(bytes) => {
                for &b in bytes {
                    out.push(Argument::Inmm8(b));
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
        return Err(AssemblerError::parse(
            line,
            col,
            "db requires at least one operand",
        ));
    }
    Ok(out)
}

fn parse_dh(parser: &mut Parser, line: u32, col: u32) -> Result<Vec<Argument>> {
    let mut out = Vec::new();
    loop {
        let tok = parser.peek(0).clone();
        match &tok.kind {
            TokenKind::Number(n) => {
                let n = *n;
                if n > 0xFFFF {
                    return Err(AssemblerError::parse(
                        tok.line,
                        tok.col,
                        "dh value out of u16 range",
                    ));
                }
                out.push(Argument::Inmm16(n as u16));
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
        return Err(AssemblerError::parse(
            line,
            col,
            "dh requires at least one operand",
        ));
    }
    Ok(out)
}

fn parse_dw(parser: &mut Parser, line: u32, col: u32) -> Result<Vec<Argument>> {
    let mut out = Vec::new();
    loop {
        let tok = parser.peek(0).clone();
        match &tok.kind {
            TokenKind::Number(n) => {
                out.push(Argument::Inmm32(*n));
                parser.advance();
            }
            TokenKind::Ident(name) => {
                out.push(Argument::UnresolvedLabel(name.clone(), line, col));
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
        return Err(AssemblerError::parse(
            line,
            col,
            "dw requires at least one operand",
        ));
    }
    Ok(out)
}

// Instruction operand parsing

fn parse_label(parser: &mut Parser, line: u32, col: u32) -> Result<Argument> {
    let tok = parser.peek(0).clone();
    if matches!(tok.kind, TokenKind::Dot) {
        parser.advance(); // consume '.'
        let (local, lline, lcol) = parser.expect_ident()?;
        let global = parser.current_global.as_deref().ok_or_else(|| {
            AssemblerError::parse(
                lline,
                lcol,
                "Local label reference without a preceding global label",
            )
        })?;
        let full_name = format!("{global}.{local}");
        Ok(Argument::UnresolvedLabel(full_name, lline, lcol))
    } else if let TokenKind::Ident(_) = &tok.kind {
        let (name, line, col) = parser.expect_ident()?;
        Ok(Argument::UnresolvedLabel(name, line, col))
    } else {
        Err(AssemblerError::parse(line, col, "Expected label"))
    }
}

fn parse_imm_or_label(parser: &mut Parser) -> Result<Argument> {
    let tok = parser.peek(0).clone();
    match &tok.kind {
        TokenKind::Number(n) => {
            parser.advance();
            Ok(Argument::Inmm32(*n))
        }
        TokenKind::Char(c) => {
            parser.advance();
            Ok(Argument::Inmm8(*c))
        }
        TokenKind::Dot | TokenKind::Ident(_) => parse_label(parser, tok.line, tok.col),
        _ => Err(AssemblerError::parse(
            tok.line,
            tok.col,
            "Expected immediate or label",
        )),
    }
}

fn parse_reg_or_imm(parser: &mut Parser, line: u32, col: u32) -> Result<Argument> {
    let tok = parser.peek(0).clone();
    if let Some(reg) = parse_register(&tok) {
        parser.advance();
        return Ok(Argument::Register(reg));
    }
    match &tok.kind {
        TokenKind::Number(n) => {
            parser.advance();
            Ok(Argument::Inmm32(*n))
        }
        TokenKind::Char(c) => {
            parser.advance();
            Ok(Argument::Inmm8(*c))
        }
        TokenKind::Dot | TokenKind::Ident(_) => parse_label(parser, tok.line, tok.col),
        _ => Err(AssemblerError::parse(
            line,
            col,
            "Expected register, immediate, or label",
        )),
    }
}

fn parse_shift_amount(parser: &mut Parser, mnemonic: &str) -> Result<Argument> {
    let tok = parser.peek(0).clone();
    if let Some(reg) = parse_register(&tok) {
        if !reg.is_rb() {
            return Err(AssemblerError::parse(
                tok.line,
                tok.col,
                format!("{mnemonic}: shift amount register must be rb"),
            ));
        }
        parser.advance();
        return Ok(Argument::Register(reg));
    }

    match tok.kind {
        TokenKind::Number(n) => {
            if n > 0xFF {
                return Err(AssemblerError::parse(
                    tok.line,
                    tok.col,
                    format!("{mnemonic}: shift amount immediate must fit in u8"),
                ));
            }
            parser.advance();
            Ok(Argument::Inmm8(n as u8))
        }
        TokenKind::Char(c) => {
            parser.advance();
            Ok(Argument::Inmm8(c))
        }
        _ => Err(AssemblerError::parse(
            tok.line,
            tok.col,
            format!("{mnemonic}: expected rb register or imm8 shift amount"),
        )),
    }
}

fn parse_u32_or_rw(parser: &mut Parser, mnemonic: &str, operand_name: &str) -> Result<Argument> {
    let tok = parser.peek(0).clone();
    if let Some(reg) = parse_register(&tok) {
        if !reg.is_rw() {
            return Err(AssemblerError::parse(
                tok.line,
                tok.col,
                format!("{mnemonic}: {operand_name} register must be rw"),
            ));
        }
        parser.advance();
        return Ok(Argument::Register(reg));
    }

    match tok.kind {
        TokenKind::Number(n) => {
            parser.advance();
            Ok(Argument::Inmm32(n))
        }
        _ => Err(AssemblerError::parse(
            tok.line,
            tok.col,
            format!("{mnemonic}: expected rw register or imm32 for {operand_name}"),
        )),
    }
}

// Narrow an Inmm32 to Inmm8/Inmm16 based on destination register width,
// or keep it as Inmm32. Returns an error if the value is out of range.
fn fit_imm(arg: Argument, width: u8, line: u32, col: u32) -> Result<Argument> {
    match arg {
        Argument::Inmm32(n) => match width {
            1 => {
                if n > 0xFF {
                    return Err(AssemblerError::parse(
                        line,
                        col,
                        "Immediate out of range for 8-bit register",
                    ));
                }
                Ok(Argument::Inmm8(n as u8))
            }
            2 => {
                if n > 0xFFFF {
                    return Err(AssemblerError::parse(
                        line,
                        col,
                        "Immediate out of range for 16-bit register",
                    ));
                }
                Ok(Argument::Inmm16(n as u16))
            }
            _ => Ok(Argument::Inmm32(n)),
        },
        other => Ok(other),
    }
}

// Opcode selection helpers: pick the reg-reg or reg-imm variant.

fn pick_alu(reg_op: Op, imm_op: Op, src: &Argument) -> Op {
    match src {
        Argument::Register(_) => reg_op,
        _ => imm_op,
    }
}

fn pick_jump(label_op: Op, reg_op: Op, target: &Argument) -> Op {
    match target {
        Argument::Register(_) => reg_op,
        _ => label_op,
    }
}

// Instruction parsing

fn parse_instruction(
    parser: &mut Parser,
    mnemonic: &str,
    line: u32,
    col: u32,
) -> Result<Instruction> {
    let no_args = [Argument::None, Argument::None, Argument::None];

    macro_rules! inst {
        ($op:expr) => {
            Instruction::new($op, no_args, 0)
        };
        ($op:expr, $a:expr) => {
            Instruction::new($op, [$a, Argument::None, Argument::None], 1)
        };
        ($op:expr, $a:expr, $b:expr) => {
            Instruction::new($op, [$a, $b, Argument::None], 2)
        };
        ($op:expr, $a:expr, $b:expr, $c:expr) => {
            Instruction::new($op, [$a, $b, $c], 3)
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
            let reg = parse_register(&tok).ok_or_else(|| {
                AssemblerError::parse(tok.line, tok.col, "PUSH expects a register")
            })?;
            parser.advance();
            Ok(inst!(Op::Push, Argument::Register(reg)))
        }

        "POP" => {
            let tok = parser.peek(0).clone();
            let reg = parse_register(&tok).ok_or_else(|| {
                AssemblerError::parse(tok.line, tok.col, "POP expects a register")
            })?;
            parser.advance();
            Ok(inst!(Op::Pop, Argument::Register(reg)))
        }

        "NOT" => {
            let tok = parser.peek(0).clone();
            let reg = parse_register(&tok).ok_or_else(|| {
                AssemblerError::parse(tok.line, tok.col, "NOT expects a register")
            })?;
            parser.advance();
            Ok(inst!(Op::Not, Argument::Register(reg)))
        }

        "MOV" => {
            let dst_tok = parser.peek(0).clone();
            let dst = parse_register(&dst_tok).ok_or_else(|| {
                AssemblerError::parse(
                    dst_tok.line,
                    dst_tok.col,
                    "MOV: expected destination register",
                )
            })?;
            parser.advance();
            parser.expect_comma()?;

            let src_tok = parser.peek(0).clone();
            if let Some(src_reg) = parse_register(&src_tok) {
                parser.advance();
                if dst.width_bytes() != src_reg.width_bytes() && !dst.is_sp() && !src_reg.is_sp() {
                    return Err(AssemblerError::parse(
                        src_tok.line,
                        src_tok.col,
                        "MOV: register views must have the same width",
                    ));
                }
                Ok(inst!(
                    Op::MovRegReg,
                    Argument::Register(dst),
                    Argument::Register(src_reg)
                ))
            } else {
                let imm = parse_imm_or_label(parser)?;
                let imm = fit_imm(imm, dst.width_bytes(), src_tok.line, src_tok.col)?;
                Ok(inst!(Op::MovRegImm, Argument::Register(dst), imm))
            }
        }

        "ZEXT" => {
            let dst_tok = parser.peek(0).clone();
            let dst = parse_register(&dst_tok).ok_or_else(|| {
                AssemblerError::parse(
                    dst_tok.line,
                    dst_tok.col,
                    "ZEXT: expected destination register",
                )
            })?;
            parser.advance();
            parser.expect_comma()?;
            let src_tok = parser.peek(0).clone();
            let src = parse_register(&src_tok).ok_or_else(|| {
                AssemblerError::parse(src_tok.line, src_tok.col, "ZEXT: expected source register")
            })?;
            parser.advance();
            if dst.width_bytes() <= src.width_bytes() {
                return Err(AssemblerError::parse(
                    src_tok.line,
                    src_tok.col,
                    "ZEXT: destination must be wider than source",
                ));
            }
            Ok(inst!(
                Op::ZeroExtend,
                Argument::Register(dst),
                Argument::Register(src)
            ))
        }

        "SEXT" => {
            let dst_tok = parser.peek(0).clone();
            let dst = parse_register(&dst_tok).ok_or_else(|| {
                AssemblerError::parse(
                    dst_tok.line,
                    dst_tok.col,
                    "SEXT: expected destination register",
                )
            })?;
            parser.advance();
            parser.expect_comma()?;
            let src_tok = parser.peek(0).clone();
            let src = parse_register(&src_tok).ok_or_else(|| {
                AssemblerError::parse(src_tok.line, src_tok.col, "SEXT: expected source register")
            })?;
            parser.advance();
            if dst.width_bytes() <= src.width_bytes() {
                return Err(AssemblerError::parse(
                    src_tok.line,
                    src_tok.col,
                    "SEXT: destination must be wider than source",
                ));
            }
            Ok(inst!(
                Op::SignExtend,
                Argument::Register(dst),
                Argument::Register(src)
            ))
        }

        op @ ("ADD" | "SUB" | "AND" | "OR" | "XOR" | "CMP") => {
            let dst_tok = parser.peek(0).clone();
            let dst = parse_register(&dst_tok).ok_or_else(|| {
                AssemblerError::parse(
                    dst_tok.line,
                    dst_tok.col,
                    format!("{op}: expected destination register"),
                )
            })?;
            parser.advance();
            parser.expect_comma()?;

            let src_tok = parser.peek(0).clone();
            let src = parse_reg_or_imm(parser, src_tok.line, src_tok.col)?;
            let src = fit_imm(src, dst.width_bytes(), src_tok.line, src_tok.col)?;

            let (reg_op, imm_op) = match op {
                "ADD" => (Op::Add, Op::AddImm),
                "SUB" => (Op::Sub, Op::SubImm),
                "AND" => (Op::And, Op::AndImm),
                "OR" => (Op::Or, Op::OrImm),
                "XOR" => (Op::Xor, Op::XorImm),
                "CMP" => (Op::Cmp, Op::CmpImm),
                _ => unreachable!(),
            };
            let op = pick_alu(reg_op, imm_op, &src);
            Ok(inst!(op, Argument::Register(dst), src))
        }

        op @ ("SHL" | "SHR" | "SAR" | "ROL" | "ROR") => {
            let dst_tok = parser.peek(0).clone();
            let dst = parse_register(&dst_tok).ok_or_else(|| {
                AssemblerError::parse(
                    dst_tok.line,
                    dst_tok.col,
                    format!("{op}: expected destination register"),
                )
            })?;
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
            Ok(inst!(op, Argument::Register(dst), src))
        }

        op @ ("JMP" | "JZ" | "JNZ" | "JC" | "JN") => {
            let tok = parser.peek(0).clone();
            let target = parse_reg_or_imm(parser, tok.line, tok.col)?;
            let (label_op, reg_op) = match op {
                "JMP" => (Op::Jmp, Op::JmpReg),
                "JZ" => (Op::Jz, Op::JzReg),
                "JNZ" => (Op::Jnz, Op::JnzReg),
                "JC" => (Op::Jc, Op::JcReg),
                "JN" => (Op::Jn, Op::JnReg),
                _ => unreachable!(),
            };
            let op = pick_jump(label_op, reg_op, &target);
            Ok(inst!(op, target))
        }

        "CALL" => {
            let tok = parser.peek(0).clone();
            let target = parse_reg_or_imm(parser, tok.line, tok.col)?;
            let op = pick_jump(Op::Call, Op::CallReg, &target);
            Ok(inst!(op, target))
        }

        "IN" => {
            let dst_tok = parser.peek(0).clone();
            let dst = parse_register(&dst_tok).ok_or_else(|| {
                AssemblerError::parse(
                    dst_tok.line,
                    dst_tok.col,
                    "IN: expected destination register",
                )
            })?;
            parser.advance();
            parser.expect_comma()?;
            let port_tok = parser.peek(0).clone();
            let port = match &port_tok.kind {
                TokenKind::Number(n) => {
                    if *n > 0xFF {
                        return Err(AssemblerError::parse(
                            port_tok.line,
                            port_tok.col,
                            "IN: port must fit in u8",
                        ));
                    }
                    *n as u8
                }
                _ => {
                    return Err(AssemblerError::parse(
                        port_tok.line,
                        port_tok.col,
                        "IN: expected port number",
                    ));
                }
            };
            parser.advance();
            Ok(inst!(
                Op::In,
                Argument::Register(dst),
                Argument::Inmm8(port)
            ))
        }

        "OUT" => {
            let port_tok = parser.peek(0).clone();
            let port = match &port_tok.kind {
                TokenKind::Number(n) => {
                    if *n > 0xFF {
                        return Err(AssemblerError::parse(
                            port_tok.line,
                            port_tok.col,
                            "OUT: port must fit in u8",
                        ));
                    }
                    *n as u8
                }
                _ => {
                    return Err(AssemblerError::parse(
                        port_tok.line,
                        port_tok.col,
                        "OUT: expected port number",
                    ));
                }
            };
            parser.advance();
            parser.expect_comma()?;
            let src_tok = parser.peek(0).clone();
            let src = parse_register(&src_tok).ok_or_else(|| {
                AssemblerError::parse(src_tok.line, src_tok.col, "OUT: expected source register")
            })?;
            parser.advance();
            Ok(inst!(
                Op::Out,
                Argument::Inmm8(port),
                Argument::Register(src)
            ))
        }

        "LOAD" => {
            let dst_tok = parser.peek(0).clone();
            let dst = parse_register(&dst_tok).ok_or_else(|| {
                AssemblerError::parse(
                    dst_tok.line,
                    dst_tok.col,
                    "LOAD: expected destination register",
                )
            })?;
            parser.advance();
            parser.expect_comma()?;
            let addr_tok = parser.peek(0).clone();
            let addr = parse_register(&addr_tok).ok_or_else(|| {
                AssemblerError::parse(
                    addr_tok.line,
                    addr_tok.col,
                    "LOAD: expected address register",
                )
            })?;
            parser.advance();
            if !addr.is_rw() {
                return Err(AssemblerError::parse(
                    addr_tok.line,
                    addr_tok.col,
                    "LOAD: address register must be rw",
                ));
            }
            Ok(inst!(
                Op::Load,
                Argument::Register(dst),
                Argument::Register(addr)
            ))
        }

        "STORE" => {
            let addr_tok = parser.peek(0).clone();
            let addr = parse_register(&addr_tok).ok_or_else(|| {
                AssemblerError::parse(
                    addr_tok.line,
                    addr_tok.col,
                    "STORE: expected address register",
                )
            })?;
            parser.advance();
            if !addr.is_rw() {
                return Err(AssemblerError::parse(
                    addr_tok.line,
                    addr_tok.col,
                    "STORE: address register must be rw",
                ));
            }
            parser.expect_comma()?;
            let src_tok = parser.peek(0).clone();
            let src = parse_register(&src_tok).ok_or_else(|| {
                AssemblerError::parse(src_tok.line, src_tok.col, "STORE: expected source register")
            })?;
            parser.advance();
            Ok(inst!(
                Op::Store,
                Argument::Register(addr),
                Argument::Register(src)
            ))
        }

        "SIE" => {
            let idx_tok = parser.peek(0).clone();
            let idx = parse_register(&idx_tok).ok_or_else(|| {
                AssemblerError::parse(idx_tok.line, idx_tok.col, "SIE: expected index register")
            })?;
            if !idx.is_rb() {
                return Err(AssemblerError::parse(
                    idx_tok.line,
                    idx_tok.col,
                    "SIE: index register must be rb",
                ));
            }
            parser.advance();
            parser.expect_comma()?;
            let addr_tok = parser.peek(0).clone();
            if let Some(addr_reg) = parse_register(&addr_tok) {
                if !addr_reg.is_rw() {
                    return Err(AssemblerError::parse(
                        addr_tok.line,
                        addr_tok.col,
                        "SIE: handler register must be rw",
                    ));
                }
                parser.advance();
                Ok(inst!(
                    Op::SieRegReg,
                    Argument::Register(idx),
                    Argument::Register(addr_reg)
                ))
            } else {
                let imm = parse_imm_or_label(parser)?;
                Ok(inst!(Op::SieRegImm, Argument::Register(idx), imm))
            }
        }

        "INT" => {
            let tok = parser.peek(0).clone();
            if let Some(reg) = parse_register(&tok) {
                if !reg.is_rb() {
                    return Err(AssemblerError::parse(
                        tok.line,
                        tok.col,
                        "INT: register form requires rb",
                    ));
                }
                parser.advance();
                Ok(inst!(Op::IntReg, Argument::Register(reg)))
            } else if let TokenKind::Number(n) = tok.kind {
                parser.advance();
                if n > 0xFF {
                    return Err(AssemblerError::parse(
                        tok.line,
                        tok.col,
                        "INT: vector index must fit in u8",
                    ));
                }
                Ok(inst!(Op::IntImm, Argument::Inmm8(n as u8)))
            } else {
                Err(AssemblerError::parse(
                    tok.line,
                    tok.col,
                    "INT: expected vector index or register",
                ))
            }
        }

        op @ ("TUR" | "TKR") => {
            let dst_tok = parser.peek(0).clone();
            let dst = parse_register(&dst_tok).ok_or_else(|| {
                AssemblerError::parse(
                    dst_tok.line,
                    dst_tok.col,
                    format!("{op}: expected destination register"),
                )
            })?;
            if !dst.is_rw() {
                return Err(AssemblerError::parse(
                    dst_tok.line,
                    dst_tok.col,
                    format!("{op}: destination register must be rw"),
                ));
            }
            parser.advance();
            parser.expect_comma()?;

            let src_tok = parser.peek(0).clone();
            let src = parse_register(&src_tok).ok_or_else(|| {
                AssemblerError::parse(
                    src_tok.line,
                    src_tok.col,
                    format!("{op}: expected source register"),
                )
            })?;
            if !src.is_rw() {
                return Err(AssemblerError::parse(
                    src_tok.line,
                    src_tok.col,
                    format!("{op}: source register must be rw"),
                ));
            }
            parser.advance();

            let opcode = match op {
                "TUR" => Op::Tur,
                "TKR" => Op::Tkr,
                _ => unreachable!(),
            };
            Ok(inst!(
                opcode,
                Argument::Register(dst),
                Argument::Register(src)
            ))
        }

        "MMAP" => {
            let virt_tok = parser.peek(0).clone();
            let virt = parse_register(&virt_tok).ok_or_else(|| {
                AssemblerError::parse(
                    virt_tok.line,
                    virt_tok.col,
                    "MMAP: expected virtual address register",
                )
            })?;
            if !virt.is_rw() {
                return Err(AssemblerError::parse(
                    virt_tok.line,
                    virt_tok.col,
                    "MMAP: virtual address register must be rw",
                ));
            }
            parser.advance();
            parser.expect_comma()?;

            let phys_tok = parser.peek(0).clone();
            let phys = parse_register(&phys_tok).ok_or_else(|| {
                AssemblerError::parse(
                    phys_tok.line,
                    phys_tok.col,
                    "MMAP: expected physical address register",
                )
            })?;
            if !phys.is_rw() {
                return Err(AssemblerError::parse(
                    phys_tok.line,
                    phys_tok.col,
                    "MMAP: physical address register must be rw",
                ));
            }
            parser.advance();
            parser.expect_comma()?;

            let size = parse_u32_or_rw(parser, "MMAP", "size")?;
            let opcode = pick_alu(Op::MmapRegRegReg, Op::MmapRegRegImm, &size);
            Ok(inst!(
                opcode,
                Argument::Register(virt),
                Argument::Register(phys),
                size
            ))
        }

        "MUNMAP" => {
            let virt_tok = parser.peek(0).clone();
            let virt = parse_register(&virt_tok).ok_or_else(|| {
                AssemblerError::parse(
                    virt_tok.line,
                    virt_tok.col,
                    "MUNMAP: expected virtual address register",
                )
            })?;
            if !virt.is_rw() {
                return Err(AssemblerError::parse(
                    virt_tok.line,
                    virt_tok.col,
                    "MUNMAP: virtual address register must be rw",
                ));
            }
            parser.advance();
            parser.expect_comma()?;

            let size = parse_u32_or_rw(parser, "MUNMAP", "size")?;
            let opcode = pick_alu(Op::MunmapRegReg, Op::MunmapRegImm, &size);
            Ok(inst!(opcode, Argument::Register(virt), size))
        }

        "MPROTECT" => {
            let virt_tok = parser.peek(0).clone();
            let virt = parse_register(&virt_tok).ok_or_else(|| {
                AssemblerError::parse(
                    virt_tok.line,
                    virt_tok.col,
                    "MPROTECT: expected virtual address register",
                )
            })?;
            if !virt.is_rw() {
                return Err(AssemblerError::parse(
                    virt_tok.line,
                    virt_tok.col,
                    "MPROTECT: virtual address register must be rw",
                ));
            }
            parser.advance();
            parser.expect_comma()?;

            let perms_tok = parser.peek(0).clone();
            let perms = parse_register(&perms_tok).ok_or_else(|| {
                AssemblerError::parse(
                    perms_tok.line,
                    perms_tok.col,
                    "MPROTECT: expected permissions register",
                )
            })?;
            if !perms.is_rb() {
                return Err(AssemblerError::parse(
                    perms_tok.line,
                    perms_tok.col,
                    "MPROTECT: permissions register must be rb",
                ));
            }
            parser.advance();
            Ok(inst!(
                Op::Mprotect,
                Argument::Register(virt),
                Argument::Register(perms)
            ))
        }

        _ => Err(AssemblerError::parse(
            line,
            col,
            format!("Unknown mnemonic: {mnemonic}"),
        )),
    }
}

// Section directive parsing

fn parse_section_directive(parser: &mut Parser) -> Result<Section> {
    let (name, nline, ncol) = parser.expect_ident()?;
    match name.as_str() {
        "rodata" => Ok(Section::RoData),
        "code" => Ok(Section::Code),
        "data" => Ok(Section::Data),
        _ => Err(AssemblerError::parse(
            nline,
            ncol,
            format!("Unknown section: .{name}"),
        )),
    }
}

// Dot-prefixed token handling: section directive or local label definition.

fn parse_dot_item(parser: &mut Parser) -> Result<DotItem> {
    // Peek ahead: if it's ident followed by ':', it's a local label definition.
    let is_local_label = matches!(parser.peek(0).kind, TokenKind::Ident(_))
        && matches!(parser.peek(1).kind, TokenKind::Colon);

    if is_local_label {
        let (local, lline, lcol) = parser.expect_ident()?;
        parser.advance(); // consume ':'
        let global = parser.current_global.as_deref().ok_or_else(|| {
            AssemblerError::parse(lline, lcol, "Local label without a preceding global label")
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
    let mut rodata: Vec<Argument> = Vec::new();
    let mut code: Vec<Instruction> = Vec::new();
    let mut data: Vec<Argument> = Vec::new();
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
                                tok.line,
                                tok.col,
                                format!("Duplicate label: {full_name}"),
                            ));
                        }
                        labels.insert(full_name, (section, offset));
                    }
                }
            }

            // Global label: ident ':'
            TokenKind::Ident(_) if matches!(parser.peek(1).kind, TokenKind::Colon) => {
                let (name, lline, lcol) = parser.expect_ident()?;
                parser.advance(); // consume ':'

                let offset = match section {
                    Section::RoData => data_byte_offset(&rodata, rodata.len()),
                    Section::Code => code.iter().map(|i| i.size as u32).sum(),
                    Section::Data => data_byte_offset(&data, data.len()),
                };

                if labels.contains_key(&name) {
                    return Err(AssemblerError::parse(
                        lline,
                        lcol,
                        format!("Duplicate label: {name}"),
                    ));
                }
                labels.insert(name.clone(), (section, offset));
                parser.current_global = Some(name);
            }

            // Data directive or instruction mnemonic
            TokenKind::Ident(ident) => {
                let ident = ident.clone();
                let iline = tok.line;
                let icol = tok.col;
                parser.advance();

                match ident.as_str() {
                    "db" | "dh" | "dw" => {
                        if matches!(section, Section::Code) {
                            return Err(AssemblerError::parse(
                                iline,
                                icol,
                                "Data directives are not allowed in .code",
                            ));
                        }
                        let args = match ident.as_str() {
                            "db" => parse_db(&mut parser, iline, icol)?,
                            "dh" => parse_dh(&mut parser, iline, icol)?,
                            "dw" => parse_dw(&mut parser, iline, icol)?,
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
                                iline,
                                icol,
                                "Instructions are only allowed in .code",
                            ));
                        }
                        let inst = parse_instruction(&mut parser, mnemonic, iline, icol)?;
                        parser.expect_eol()?;
                        code.push(inst);
                    }
                }
            }

            _ => {
                return Err(AssemblerError::parse(
                    tok.line,
                    tok.col,
                    format!("Unexpected token: {:?}", tok.kind),
                ));
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
