use fvm_core::{
    argument::Argument, instruction::Instruction, opcode::Op, register::RegisterEncoding,
};

use crate::{
    error::{VmError, VmResult},
    vm::VM,
};

pub fn decode_instruction(vm: &mut VM, opcode: Op) -> VmResult<Instruction> {
    let ip = vm.files[vm.active].ip;

    let instruction = match opcode {
        Op::Nop | Op::Halt | Op::Ret | Op::Iret | Op::Dpl => {
            Instruction::new(opcode, none_args(), 0)
        }

        Op::Push
        | Op::Pop
        | Op::Not
        | Op::JmpReg
        | Op::JzReg
        | Op::JnzReg
        | Op::JcReg
        | Op::JnReg
        | Op::CallReg
        | Op::IntReg => Instruction::new(
            opcode,
            [read_reg(vm, ip, 1)?, Argument::None, Argument::None],
            1,
        ),

        Op::Jmp | Op::Jz | Op::Jnz | Op::Jc | Op::Jn | Op::Call => Instruction::new(
            opcode,
            [read_u32_arg(vm, ip, 1)?, Argument::None, Argument::None],
            1,
        ),

        Op::IntImm => Instruction::new(
            opcode,
            [
                Argument::Inmm8(read_exec_byte(vm, ip, 1)?),
                Argument::None,
                Argument::None,
            ],
            1,
        ),

        Op::MovRegReg
        | Op::ZeroExtend
        | Op::SignExtend
        | Op::Add
        | Op::Sub
        | Op::And
        | Op::Or
        | Op::Xor
        | Op::Cmp
        | Op::Load
        | Op::Store
        | Op::SieRegReg
        | Op::Tur
        | Op::Tkr => Instruction::new(
            opcode,
            [read_reg(vm, ip, 1)?, read_reg(vm, ip, 2)?, Argument::None],
            2,
        ),

        Op::MovRegImm
        | Op::AddImm
        | Op::SubImm
        | Op::AndImm
        | Op::OrImm
        | Op::XorImm
        | Op::CmpImm => {
            let dst = read_reg_encoding(vm, ip, 1)?;
            let imm = read_width_immediate(vm, ip, 2, dst.width_bytes())?;
            Instruction::new(opcode, [Argument::Register(dst), imm, Argument::None], 2)
        }

        Op::In => Instruction::new(
            opcode,
            [
                read_reg(vm, ip, 1)?,
                Argument::Inmm8(read_exec_byte(vm, ip, 2)?),
                Argument::None,
            ],
            2,
        ),

        Op::Out => Instruction::new(
            opcode,
            [
                Argument::Inmm8(read_exec_byte(vm, ip, 1)?),
                read_reg(vm, ip, 2)?,
                Argument::None,
            ],
            2,
        ),

        Op::SieRegImm => Instruction::new(
            opcode,
            [
                read_reg(vm, ip, 1)?,
                read_u32_arg(vm, ip, 2)?,
                Argument::None,
            ],
            2,
        ),

        Op::ShlReg | Op::ShrReg | Op::SarReg | Op::RolReg | Op::RorReg => Instruction::new(
            opcode,
            [read_reg(vm, ip, 1)?, read_reg(vm, ip, 2)?, Argument::None],
            2,
        ),

        Op::ShlImm | Op::ShrImm | Op::SarImm | Op::RolImm | Op::RorImm => Instruction::new(
            opcode,
            [
                read_reg(vm, ip, 1)?,
                Argument::Inmm8(read_exec_byte(vm, ip, 2)?),
                Argument::None,
            ],
            2,
        ),

        Op::MmapRegRegReg => Instruction::new(
            opcode,
            [
                read_reg(vm, ip, 1)?,
                read_reg(vm, ip, 2)?,
                read_reg(vm, ip, 3)?,
            ],
            3,
        ),

        Op::MmapRegRegImm => Instruction::new(
            opcode,
            [
                read_reg(vm, ip, 1)?,
                read_reg(vm, ip, 2)?,
                read_u32_arg(vm, ip, 3)?,
            ],
            3,
        ),

        Op::MunmapRegReg => Instruction::new(
            opcode,
            [read_reg(vm, ip, 1)?, read_reg(vm, ip, 2)?, Argument::None],
            2,
        ),

        Op::MunmapRegImm => Instruction::new(
            opcode,
            [
                read_reg(vm, ip, 1)?,
                read_u32_arg(vm, ip, 2)?,
                Argument::None,
            ],
            2,
        ),

        Op::Mprotect => Instruction::new(
            opcode,
            [read_reg(vm, ip, 1)?, read_reg(vm, ip, 2)?, Argument::None],
            2,
        ),
    };

    Ok(instruction)
}

fn none_args() -> [Argument; 3] {
    [Argument::None, Argument::None, Argument::None]
}

fn read_reg(vm: &VM, ip: u32, offset: u32) -> VmResult<Argument> {
    Ok(Argument::Register(read_reg_encoding(vm, ip, offset)?))
}

fn read_reg_encoding(vm: &VM, ip: u32, offset: u32) -> VmResult<RegisterEncoding> {
    let byte = read_exec_byte(vm, ip, offset)?;
    RegisterEncoding::from_byte(byte).ok_or(VmError::InvalidOpcode {
        opcode: byte,
        address: ip.wrapping_add(offset),
    })
}

fn read_u32_arg(vm: &VM, ip: u32, offset: u32) -> VmResult<Argument> {
    Ok(Argument::Inmm32(read_exec_u32(vm, ip, offset)?))
}

fn read_width_immediate(vm: &VM, ip: u32, offset: u32, width: u8) -> VmResult<Argument> {
    match width {
        1 => Ok(Argument::Inmm8(read_exec_byte(vm, ip, offset)?)),
        2 => Ok(Argument::Inmm16(read_exec_u16(vm, ip, offset)?)),
        4 => Ok(Argument::Inmm32(read_exec_u32(vm, ip, offset)?)),
        _ => Err(VmError::InvalidRomImage(format!(
            "unsupported immediate width {width}"
        ))),
    }
}

fn read_exec_byte(vm: &VM, ip: u32, offset: u32) -> VmResult<u8> {
    vm.bus.fetch_byte(add_offset(ip, offset)?)
}

fn read_exec_u16(vm: &VM, ip: u32, offset: u32) -> VmResult<u16> {
    let hi = read_exec_byte(vm, ip, offset)? as u16;
    let lo = read_exec_byte(vm, ip, offset + 1)? as u16;
    Ok((hi << 8) | lo)
}

fn read_exec_u32(vm: &VM, ip: u32, offset: u32) -> VmResult<u32> {
    let b0 = read_exec_byte(vm, ip, offset)? as u32;
    let b1 = read_exec_byte(vm, ip, offset + 1)? as u32;
    let b2 = read_exec_byte(vm, ip, offset + 2)? as u32;
    let b3 = read_exec_byte(vm, ip, offset + 3)? as u32;
    Ok((b0 << 24) | (b1 << 16) | (b2 << 8) | b3)
}

fn add_offset(base: u32, offset: u32) -> VmResult<u32> {
    base.checked_add(offset).ok_or(VmError::AddressOverflow)
}
