use std::rc::Rc;

use fvm_core::{
    argument::Argument, instruction::Instruction, opcode::Op, register::RegisterEncoding,
};

use crate::{
    error::{VmError, VmResult},
    vm::{
        KERNEL_CONTEXT, USER_CONTEXT, VM, bus, device::PortMappedDevice, flags::Flag,
        interrupts::Interrupt,
    },
};

pub fn execute_instruction(vm: &mut VM, instruction: Instruction) -> VmResult<()> {
    match instruction.opcode {
        Op::Nop => Ok(()),
        Op::Halt => {
            vm.halted = true;
            Ok(())
        }
        Op::Push => push_register(vm, &instruction.arguments[0]),
        Op::Pop => pop_register(vm, &instruction.arguments[0]),
        Op::MovRegImm => move_immediate(vm, &instruction.arguments[0], &instruction.arguments[1]),
        Op::MovRegReg => move_register(vm, &instruction.arguments[0], &instruction.arguments[1]),
        Op::ZeroExtend => zero_extend(vm, &instruction.arguments[0], &instruction.arguments[1]),
        Op::SignExtend => sign_extend(vm, &instruction.arguments[0], &instruction.arguments[1]),
        Op::Add | Op::AddImm => arithmetic(vm, &instruction, ArithmeticOp::Add),
        Op::Sub | Op::SubImm => arithmetic(vm, &instruction, ArithmeticOp::Sub),
        Op::And | Op::AndImm => arithmetic(vm, &instruction, ArithmeticOp::And),
        Op::Or | Op::OrImm => arithmetic(vm, &instruction, ArithmeticOp::Or),
        Op::Xor | Op::XorImm => arithmetic(vm, &instruction, ArithmeticOp::Xor),
        Op::Not => bitwise_not(vm, &instruction.arguments[0]),
        Op::Cmp | Op::CmpImm => compare(vm, &instruction.arguments[0], &instruction.arguments[1]),
        Op::Mul | Op::MulImm => arithmetic(vm, &instruction, ArithmeticOp::Mul),
        Op::Div | Op::DivImm => arithmetic(vm, &instruction, ArithmeticOp::Div),
        Op::Mod | Op::ModImm => arithmetic(vm, &instruction, ArithmeticOp::Mod),
        Op::Smul | Op::SmulImm => arithmetic(vm, &instruction, ArithmeticOp::Smul),
        Op::Sdiv | Op::SdivImm => arithmetic(vm, &instruction, ArithmeticOp::Sdiv),
        Op::Smod | Op::SmodImm => arithmetic(vm, &instruction, ArithmeticOp::Smod),
        Op::Jmp | Op::JmpReg => jump(vm, &instruction.arguments[0]),
        Op::Jz | Op::JzReg => conditional_jump(
            vm,
            &instruction.arguments[0],
            vm.files[vm.active].flags.is_set(Flag::Zero),
            instruction.size as u32,
        ),
        Op::Jnz | Op::JnzReg => conditional_jump(
            vm,
            &instruction.arguments[0],
            !vm.files[vm.active].flags.is_set(Flag::Zero),
            instruction.size as u32,
        ),
        Op::Jc | Op::JcReg => conditional_jump(
            vm,
            &instruction.arguments[0],
            vm.files[vm.active].flags.is_set(Flag::Carry),
            instruction.size as u32,
        ),
        Op::Jn | Op::JnReg => conditional_jump(
            vm,
            &instruction.arguments[0],
            vm.files[vm.active].flags.is_set(Flag::Negative),
            instruction.size as u32,
        ),
        Op::Jo | Op::JoReg => conditional_jump(
            vm,
            &instruction.arguments[0],
            vm.files[vm.active].flags.is_set(Flag::Overflow),
            instruction.size as u32,
        ),
        Op::Jno | Op::JnoReg => conditional_jump(
            vm,
            &instruction.arguments[0],
            !vm.files[vm.active].flags.is_set(Flag::Overflow),
            instruction.size as u32,
        ),
        Op::Call | Op::CallReg => call(vm, &instruction),
        Op::Ret => ret(vm),
        Op::In => input(vm, &instruction.arguments[0], &instruction.arguments[1]),
        Op::Out => output(vm, &instruction.arguments[0], &instruction.arguments[1]),
        Op::Load => load(vm, &instruction.arguments[0], &instruction.arguments[1]),
        Op::Store => store(vm, &instruction.arguments[0], &instruction.arguments[1]),
        Op::SieRegImm | Op::SieRegReg => {
            sie(vm, &instruction.arguments[0], &instruction.arguments[1])
        }
        Op::IntImm | Op::IntReg => int_instruction(vm, &instruction),
        Op::Iret => vm.iret(),
        Op::Dpl => drop_to_user(vm),
        Op::Tur => {
            transfer_user_to_kernel(vm, &instruction.arguments[0], &instruction.arguments[1])
        }
        Op::Tkr => {
            transfer_kernel_to_user(vm, &instruction.arguments[0], &instruction.arguments[1])
        }
        Op::ShlReg | Op::ShlImm => shift(vm, &instruction, ShiftOp::Shl),
        Op::ShrReg | Op::ShrImm => shift(vm, &instruction, ShiftOp::Shr),
        Op::SarReg | Op::SarImm => shift(vm, &instruction, ShiftOp::Sar),
        Op::RolReg | Op::RolImm => shift(vm, &instruction, ShiftOp::Rol),
        Op::RorReg | Op::RorImm => shift(vm, &instruction, ShiftOp::Ror),
        Op::MmapRegRegReg | Op::MmapRegRegImm => mmap_instruction(vm, &instruction),
        Op::MunmapRegReg | Op::MunmapRegImm => munmap_instruction(vm, &instruction),
        Op::MprotectRegRegRb | Op::MprotectRegImmRb => mprotect_instruction(vm, &instruction),
    }
}

enum ArithmeticOp {
    Add,
    Sub,
    And,
    Or,
    Xor,
    Mul,
    Div,
    Mod,
    Smul,
    Sdiv,
    Smod,
}

enum ShiftOp {
    Shl,
    Shr,
    Sar,
    Rol,
    Ror,
}

fn push_register(vm: &mut VM, arg: &Argument) -> VmResult<()> {
    let reg = expect_register(arg)?;
    let width = reg.width_bytes();
    let value = read_register(vm, reg)?;
    push_value(vm, width, value)
}

fn pop_register(vm: &mut VM, arg: &Argument) -> VmResult<()> {
    let reg = expect_register(arg)?;
    let width = reg.width_bytes();
    let value = pop_value(vm, width)?;
    write_register(vm, reg, value)
}

fn move_immediate(vm: &mut VM, dst_arg: &Argument, src_arg: &Argument) -> VmResult<()> {
    let dst = expect_register(dst_arg)?;
    let value = operand_value(src_arg)?;
    write_register(vm, dst, value)
}

fn move_register(vm: &mut VM, dst_arg: &Argument, src_arg: &Argument) -> VmResult<()> {
    let dst = expect_register(dst_arg)?;
    let src = expect_register(src_arg)?;
    let value = read_register(vm, src)?;
    write_register(vm, dst, value)
}

fn zero_extend(vm: &mut VM, dst_arg: &Argument, src_arg: &Argument) -> VmResult<()> {
    let dst = expect_register(dst_arg)?;
    let src = expect_register(src_arg)?;
    write_register(
        vm,
        dst,
        read_register(vm, src)? & mask_for_width(src.width_bytes()),
    )
}

fn sign_extend(vm: &mut VM, dst_arg: &Argument, src_arg: &Argument) -> VmResult<()> {
    let dst = expect_register(dst_arg)?;
    let src = expect_register(src_arg)?;
    let width = src.width_bytes();
    let value = read_register(vm, src)? & mask_for_width(width);
    let extended = if value & sign_bit(width) != 0 {
        value | !mask_for_width(width)
    } else {
        value
    };
    write_register(vm, dst, extended)
}

fn arithmetic(vm: &mut VM, instruction: &Instruction, op: ArithmeticOp) -> VmResult<()> {
    let dst = expect_register(&instruction.arguments[0])?;
    let lhs = read_register(vm, dst)? & mask_for_width(dst.width_bytes());
    let rhs = read_source_value(vm, &instruction.arguments[1])? & mask_for_width(dst.width_bytes());
    let width = dst.width_bytes();

    let (result, carry, overflow) = match op {
        ArithmeticOp::Add => {
            let (res, carry) = add_values(lhs, rhs, width);
            let overflow = check_add_overflow(lhs, rhs, res, width, false);
            (res, carry, overflow)
        },
        ArithmeticOp::Sub => {
            let (res, carry) = sub_values(lhs, rhs, width);
            let overflow = check_sub_overflow(lhs, rhs, res, width, false);
            (res, carry, overflow)
        },
        ArithmeticOp::And => (lhs & rhs, false, false),
        ArithmeticOp::Or => (lhs | rhs, false, false),
        ArithmeticOp::Xor => (lhs ^ rhs, false, false),
        ArithmeticOp::Mul => {
            let (res, carry, overflow) = mul_values(lhs, rhs, width);
            (res, carry, overflow)
        },
        ArithmeticOp::Div => {
            if rhs == 0 {
                return Err(VmError::Interrupt(Interrupt::DivisionByZero));
            }
            (lhs / rhs, false, false)
        },
        ArithmeticOp::Mod => {
            if rhs == 0 {
                return Err(VmError::Interrupt(Interrupt::DivisionByZero));
            }
            (lhs % rhs, false, false)
        },
        ArithmeticOp::Smul => {
            let (res, carry, overflow) = smul_values(lhs, rhs, width);
            (res, carry, overflow)
        },
        ArithmeticOp::Sdiv => {
            if rhs == 0 {
                return Err(VmError::Interrupt(Interrupt::DivisionByZero));
            }
            let lhs_signed = sign_extend_to_i32(lhs, width);
            let rhs_signed = sign_extend_to_i32(rhs, width);
            let result = lhs_signed / rhs_signed;
            (result as u32, false, false)
        },
        ArithmeticOp::Smod => {
            if rhs == 0 {
                return Err(VmError::Interrupt(Interrupt::DivisionByZero));
            }
            let lhs_signed = sign_extend_to_i32(lhs, width);
            let rhs_signed = sign_extend_to_i32(rhs, width);
            let result = lhs_signed % rhs_signed;
            (result as u32, false, false)
        },
    };

    write_register(vm, dst, result)?;
    set_arithmetic_flags_with_overflow(vm, result, width, carry, overflow);
    Ok(())
}

fn bitwise_not(vm: &mut VM, arg: &Argument) -> VmResult<()> {
    let reg = expect_register(arg)?;
    let width = reg.width_bytes();
    let value = (!read_register(vm, reg)?) & mask_for_width(width);
    write_register(vm, reg, value)?;
    set_arithmetic_flags(vm, value, width, false);
    Ok(())
}

fn compare(vm: &mut VM, dst_arg: &Argument, src_arg: &Argument) -> VmResult<()> {
    let dst = expect_register(dst_arg)?;
    let lhs = read_register(vm, dst)? & mask_for_width(dst.width_bytes());
    let rhs = read_source_value(vm, src_arg)? & mask_for_width(dst.width_bytes());
    let (result, carry) = sub_values(lhs, rhs, dst.width_bytes());
    set_arithmetic_flags(vm, result, dst.width_bytes(), carry);
    Ok(())
}

fn jump(vm: &mut VM, target_arg: &Argument) -> VmResult<()> {
    vm.files[vm.active].ip = read_source_value(vm, target_arg)?;
    Ok(())
}

fn conditional_jump(
    vm: &mut VM,
    target_arg: &Argument,
    condition: bool,
    instruction_size: u32,
) -> VmResult<()> {
    if condition {
        jump(vm, target_arg)?;
    } else {
        vm.files[vm.active].ip = vm.files[vm.active].ip.wrapping_add(instruction_size);
    }
    Ok(())
}

fn call(vm: &mut VM, instruction: &Instruction) -> VmResult<()> {
    let return_ip = vm.files[vm.active].ip.wrapping_add(instruction.size as u32);
    push_value(vm, 4, return_ip)?;
    vm.files[vm.active].ip = read_source_value(vm, &instruction.arguments[0])?;
    Ok(())
}

fn ret(vm: &mut VM) -> VmResult<()> {
    vm.files[vm.active].ip = pop_value(vm, 4)?;
    Ok(())
}

fn input(vm: &mut VM, dst_arg: &Argument, port_arg: &Argument) -> VmResult<()> {
    let dst = expect_register(dst_arg)?;
    let port = operand_value(port_arg)?;
    let device = port_device(vm, port)?;
    let value = match dst.width_bytes() {
        1 => device.read_byte(port)? as u32,
        2 => device.read_half(port)? as u32,
        4 => device.read_word(port)?,
        width => {
            return Err(VmError::InvalidRomImage(format!(
                "unsupported input width {width}"
            )));
        }
    };
    write_register(vm, dst, value)
}

fn output(vm: &mut VM, port_arg: &Argument, src_arg: &Argument) -> VmResult<()> {
    let port = operand_value(port_arg)?;
    let src = expect_register(src_arg)?;
    let value = read_register(vm, src)?;
    let device = port_device(vm, port)?;

    match src.width_bytes() {
        1 => device.write_byte(port, value as u8),
        2 => device.write_half(port, value as u16),
        4 => device.write_word(port, value),
        width => Err(VmError::InvalidRomImage(format!(
            "unsupported output width {width}"
        ))),
    }
}

fn load(vm: &mut VM, dst_arg: &Argument, addr_arg: &Argument) -> VmResult<()> {
    let dst = expect_register(dst_arg)?;
    let addr = read_register(vm, expect_register(addr_arg)?)?;
    let value = match dst.width_bytes() {
        1 => vm.bus.read_byte(addr)? as u32,
        2 => vm.bus.read_u16(addr)? as u32,
        4 => vm.bus.read_u32(addr)?,
        width => {
            return Err(VmError::InvalidRomImage(format!(
                "unsupported load width {width}"
            )));
        }
    };
    write_register(vm, dst, value)
}

fn store(vm: &mut VM, addr_arg: &Argument, src_arg: &Argument) -> VmResult<()> {
    let addr = read_register(vm, expect_register(addr_arg)?)?;
    let src = expect_register(src_arg)?;
    let value = read_register(vm, src)?;

    match src.width_bytes() {
        1 => vm.bus.write_byte(addr, value as u8),
        2 => vm.bus.write_u16(addr, value as u16),
        4 => vm.bus.write_u32(addr, value),
        width => Err(VmError::InvalidRomImage(format!(
            "unsupported store width {width}"
        ))),
    }
}

fn read_source_value(vm: &VM, arg: &Argument) -> VmResult<u32> {
    match arg {
        Argument::Register(reg) => read_register(vm, *reg),
        _ => Ok(operand_value(arg)?),
    }
}

fn operand_value(arg: &Argument) -> VmResult<u32> {
    match arg {
        Argument::Inmm8(value) => Ok(*value as u32),
        Argument::Inmm16(value) => Ok(*value as u32),
        Argument::Inmm32(value) => Ok(*value),
        Argument::Label { address, .. } => Ok(*address),
        _ => Err(VmError::InvalidRomImage(
            "expected an immediate-style operand".to_string(),
        )),
    }
}

fn expect_register(arg: &Argument) -> VmResult<RegisterEncoding> {
    match arg {
        Argument::Register(reg) => Ok(*reg),
        _ => Err(VmError::InvalidRomImage(
            "expected a register operand".to_string(),
        )),
    }
}

fn read_register(vm: &VM, reg: RegisterEncoding) -> VmResult<u32> {
    let file = &vm.files[vm.active];
    let value = if reg.is_sp() {
        file.sp
    } else if reg.is_cr() {
        file.cr
    } else if reg.is_ip() {
        return Err(VmError::InvalidRomImage(
            "ip is not readable via MOV".to_string(),
        ));
    } else if reg.is_mr() {
        if vm.active != KERNEL_CONTEXT as usize {
            return Err(VmError::Interrupt(Interrupt::PrivilegeViolation));
        }
        vm.mr
    } else if reg.is_rw() {
        file.regs[reg.index() as usize]
    } else if reg.is_rh() {
        file.regs[reg.index() as usize] & 0xFFFF
    } else if reg.is_rb() {
        file.regs[reg.index() as usize] & 0xFF
    } else {
        return Err(VmError::InvalidRomImage(format!(
            "unsupported register encoding 0x{:02X}",
            reg.0
        )));
    };
    Ok(value)
}

fn write_register(vm: &mut VM, reg: RegisterEncoding, value: u32) -> VmResult<()> {
    if reg.is_cr() {
        return Err(VmError::Interrupt(Interrupt::PrivilegeViolation));
    }

    if reg.is_ip() {
        return Err(VmError::InvalidRomImage(
            "ip is not writable via MOV".to_string(),
        ));
    }

    if reg.is_mr() {
        if vm.active != KERNEL_CONTEXT as usize {
            return Err(VmError::Interrupt(Interrupt::PrivilegeViolation));
        }
        vm.mr = value;
        return Ok(());
    }

    let active = vm.active;
    let file = &mut vm.files[active];
    if reg.is_sp() {
        file.sp = value;
    } else if reg.is_rw() {
        file.regs[reg.index() as usize] = value;
    } else if reg.is_rh() {
        let slot = &mut file.regs[reg.index() as usize];
        *slot = (*slot & 0xFFFF_0000) | (value & 0xFFFF);
    } else if reg.is_rb() {
        let slot = &mut file.regs[reg.index() as usize];
        *slot = (*slot & 0xFFFF_FF00) | (value & 0xFF);
    } else {
        return Err(VmError::InvalidRomImage(format!(
            "unsupported register encoding 0x{:02X}",
            reg.0
        )));
    }
    Ok(())
}

fn push_value(vm: &mut VM, width: u8, value: u32) -> VmResult<()> {
    let active = vm.active;
    let current_sp = vm.files[active].sp;
    let new_sp = current_sp
        .checked_sub(width as u32)
        .ok_or(VmError::AddressOverflow)?;
    vm.files[active].sp = new_sp;

    match width {
        1 => vm.bus.write_byte(new_sp, value as u8),
        2 => vm.bus.write_u16(new_sp, value as u16),
        4 => vm.bus.write_u32(new_sp, value),
        _ => Err(VmError::InvalidRomImage(format!(
            "unsupported push width {width}"
        ))),
    }
}

fn pop_value(vm: &mut VM, width: u8) -> VmResult<u32> {
    let active = vm.active;
    let sp = vm.files[active].sp;
    let value = match width {
        1 => vm.bus.read_byte(sp)? as u32,
        2 => vm.bus.read_u16(sp)? as u32,
        4 => vm.bus.read_u32(sp)?,
        _ => {
            return Err(VmError::InvalidRomImage(format!(
                "unsupported pop width {width}"
            )));
        }
    };
    vm.files[active].sp = sp
        .checked_add(width as u32)
        .ok_or(VmError::AddressOverflow)?;
    Ok(value)
}

fn sie(vm: &mut VM, idx_arg: &Argument, addr_arg: &Argument) -> VmResult<()> {
    if vm.active != KERNEL_CONTEXT as usize {
        return Err(VmError::Interrupt(Interrupt::PrivilegeViolation));
    }
    let index = read_register(vm, expect_register(idx_arg)?)? as usize;
    let address = read_source_value(vm, addr_arg)?;
    vm.ivt[index] = address;
    Ok(())
}

fn int_instruction(vm: &mut VM, instruction: &Instruction) -> VmResult<()> {
    let index = read_source_value(vm, &instruction.arguments[0])? as u8;
    let resume_ip = vm.files[vm.active].ip.wrapping_add(instruction.size as u32);
    vm.raise_interrupt(index, resume_ip)
}

fn drop_to_user(vm: &mut VM) -> VmResult<()> {
    if vm.active != KERNEL_CONTEXT as usize {
        return Err(VmError::Interrupt(Interrupt::PrivilegeViolation));
    }
    vm.active = USER_CONTEXT as usize;
    Ok(())
}

fn transfer_user_to_kernel(vm: &mut VM, dst_arg: &Argument, src_arg: &Argument) -> VmResult<()> {
    if vm.active != KERNEL_CONTEXT as usize {
        return Err(VmError::Interrupt(Interrupt::PrivilegeViolation));
    }
    let dst = expect_register(dst_arg)?;
    let src = expect_register(src_arg)?;
    let source_value = if src.is_rw() {
        vm.files[USER_CONTEXT as usize].regs[src.index() as usize]
    } else if src.is_ip() {
        vm.files[USER_CONTEXT as usize].ip
    } else if src.is_cr() {
        vm.files[USER_CONTEXT as usize].cr
    } else {
        return Err(VmError::InvalidRomImage(format!(
            "TUR: invalid source register 0x{:02X}",
            src.0
        )));
    };
    vm.files[KERNEL_CONTEXT as usize].regs[dst.index() as usize] = source_value;
    Ok(())
}

fn transfer_kernel_to_user(vm: &mut VM, dst_arg: &Argument, src_arg: &Argument) -> VmResult<()> {
    if vm.active != KERNEL_CONTEXT as usize {
        return Err(VmError::Interrupt(Interrupt::PrivilegeViolation));
    }
    let dst = expect_register(dst_arg)?;
    let src = expect_register(src_arg)?;
    let source_value = vm.files[KERNEL_CONTEXT as usize].regs[src.index() as usize];
    if dst.is_rw() {
        vm.files[USER_CONTEXT as usize].regs[dst.index() as usize] = source_value;
    } else if dst.is_ip() {
        vm.files[USER_CONTEXT as usize].ip = source_value;
    } else if dst.is_cr() {
        vm.files[USER_CONTEXT as usize].cr = source_value;
    } 
    else if dst.is_sp() {
        vm.files[USER_CONTEXT as usize].sp = source_value;
    }
    else {
        return Err(VmError::InvalidRomImage(format!(
            "TKR: invalid destination register {:?}",
            dst
        )));
    }
    Ok(())
}

fn shift(vm: &mut VM, instruction: &Instruction, op: ShiftOp) -> VmResult<()> {
    let dst = expect_register(&instruction.arguments[0])?;
    let width = dst.width_bytes();
    let width_bits = width as u32 * 8;
    let value = read_register(vm, dst)? & mask_for_width(width);
    let amount = read_source_value(vm, &instruction.arguments[1])? & 0xFF;

    let (result, set_carry) = match op {
        ShiftOp::Shl => {
            if amount == 0 {
                (value, false)
            } else if amount >= width_bits {
                (0, false)
            } else {
                let carry = (value >> (width_bits - amount)) & 1 != 0;
                ((value << amount) & mask_for_width(width), carry)
            }
        }
        ShiftOp::Shr => {
            if amount == 0 {
                (value, false)
            } else if amount >= width_bits {
                (0, false)
            } else {
                let carry = (value >> (amount - 1)) & 1 != 0;
                (value >> amount, carry)
            }
        }
        ShiftOp::Sar => {
            if amount == 0 {
                (value, false)
            } else {
                let carry = if amount < width_bits {
                    (value >> (amount - 1)) & 1 != 0
                } else {
                    value & sign_bit(width) != 0
                };
                let result = if amount >= width_bits {
                    if value & sign_bit(width) != 0 {
                        mask_for_width(width)
                    } else {
                        0
                    }
                } else {
                    let signed = match width {
                        1 => (value as u8 as i8 as i32) as u32,
                        2 => (value as u16 as i16 as i32) as u32,
                        _ => value as i32 as u32,
                    };
                    ((signed as i32) >> amount) as u32 & mask_for_width(width)
                };
                (result, carry)
            }
        }
        ShiftOp::Rol => {
            let n = amount % width_bits;
            if n == 0 {
                (value, false)
            } else {
                let result = ((value << n) | (value >> (width_bits - n))) & mask_for_width(width);
                (result, false)
            }
        }
        ShiftOp::Ror => {
            let n = amount % width_bits;
            if n == 0 {
                (value, false)
            } else {
                let result = ((value >> n) | (value << (width_bits - n))) & mask_for_width(width);
                (result, false)
            }
        }
    };

    write_register(vm, dst, result)?;
    let flags = &mut vm.files[vm.active].flags;
    set_or_clear(flags, Flag::Zero, result == 0);
    set_or_clear(flags, Flag::Negative, result & sign_bit(width) != 0);
    match op {
        ShiftOp::Rol | ShiftOp::Ror => {}
        _ => set_or_clear(flags, Flag::Carry, set_carry),
    }
    Ok(())
}

fn mmap_instruction(vm: &mut VM, instruction: &Instruction) -> VmResult<()> {
    if vm.active != KERNEL_CONTEXT as usize {
        return Err(VmError::Interrupt(Interrupt::PrivilegeViolation));
    }
    let virt_page = read_register(vm, expect_register(&instruction.arguments[0])?)?;
    let phys_page = read_register(vm, expect_register(&instruction.arguments[1])?)?;
    let page_count = read_source_value(vm, &instruction.arguments[2])?;
    let context = vm.mr;
    
    vm.bus.mmap(
        context,
        virt_page,
        phys_page,
        page_count,
        bus::perm::READ | bus::perm::WRITE,
    )
}

fn munmap_instruction(vm: &mut VM, instruction: &Instruction) -> VmResult<()> {
    if vm.active != KERNEL_CONTEXT as usize {
        return Err(VmError::Interrupt(Interrupt::PrivilegeViolation));
    }
    let virt_page = read_register(vm, expect_register(&instruction.arguments[0])?)?;
    let page_count = read_source_value(vm, &instruction.arguments[1])?;
    let context = vm.mr;
    vm.bus.munmap(context, virt_page, page_count)
}

fn mprotect_instruction(vm: &mut VM, instruction: &Instruction) -> VmResult<()> {
    if vm.active != KERNEL_CONTEXT as usize {
        return Err(VmError::Interrupt(Interrupt::PrivilegeViolation));
    }
    let virt_page = read_register(vm, expect_register(&instruction.arguments[0])?)?;
    let page_count_arg = &instruction.arguments[1];
    let page_count = read_source_value(vm, page_count_arg)?;
    let perms = read_register(vm, expect_register(&instruction.arguments[2])?)? as u8;
    let context = vm.mr;
    
    vm.bus.mprotect(context, virt_page, page_count, perms)
}

fn port_device(vm: &VM, port: u32) -> VmResult<Rc<dyn PortMappedDevice>> {
    vm.port_devices
        .get(&port)
        .cloned()
        .ok_or(VmError::DeviceError {
            device: *b"PORTMAP\0",
            offset: port,
            message: format!("no device mapped to port {port}"),
        })
}

fn add_values(lhs: u32, rhs: u32, width: u8) -> (u32, bool) {
    let mask = mask_for_width(width) as u64;
    let sum = lhs as u64 + rhs as u64;
    ((sum as u32) & mask_for_width(width), sum > mask)
}

fn sub_values(lhs: u32, rhs: u32, width: u8) -> (u32, bool) {
    (lhs.wrapping_sub(rhs) & mask_for_width(width), rhs > lhs)
}

fn set_arithmetic_flags(vm: &mut VM, result: u32, width: u8, carry: bool) {
    let masked = result & mask_for_width(width);
    let flags = &mut vm.files[vm.active].flags;

    set_or_clear(flags, Flag::Zero, masked == 0);
    set_or_clear(flags, Flag::Negative, masked & sign_bit(width) != 0);
    set_or_clear(flags, Flag::Carry, carry);
}

fn set_arithmetic_flags_with_overflow(vm: &mut VM, result: u32, width: u8, carry: bool, overflow: bool) {
    let masked = result & mask_for_width(width);
    let flags = &mut vm.files[vm.active].flags;

    set_or_clear(flags, Flag::Zero, masked == 0);
    set_or_clear(flags, Flag::Negative, masked & sign_bit(width) != 0);
    set_or_clear(flags, Flag::Carry, carry);
    set_or_clear(flags, Flag::Overflow, overflow);
}

fn set_or_clear(flags: &mut crate::vm::flags::Flags, flag: Flag, value: bool) {
    if value {
        flags.set(flag);
    } else {
        flags.clear(flag);
    }
}

fn mask_for_width(width: u8) -> u32 {
    match width {
        1 => 0x0000_00FF,
        2 => 0x0000_FFFF,
        4 => 0xFFFF_FFFF,
        _ => 0,
    }
}

fn sign_bit(width: u8) -> u32 {
    match width {
        1 => 0x0000_0080,
        2 => 0x0000_8000,
        4 => 0x8000_0000,
        _ => 0,
    }
}

fn sign_extend_to_i32(value: u32, width: u8) -> i32 {
    let masked = value & mask_for_width(width);
    if masked & sign_bit(width) != 0 {
        (masked as i32) | ((!mask_for_width(width)) as i32)
    } else {
        masked as i32
    }
}

fn mul_values(lhs: u32, rhs: u32, width: u8) -> (u32, bool, bool) {
    let mask = mask_for_width(width);
    let lhs_masked = lhs & mask;
    let rhs_masked = rhs & mask;
    
    let product = lhs_masked as u64 * rhs_masked as u64;
    let result = (product & mask as u64) as u32;
    let carry = product > mask as u64;
    let overflow = carry;
    
    (result, carry, overflow)
}

fn smul_values(lhs: u32, rhs: u32, width: u8) -> (u32, bool, bool) {
    let lhs_signed = sign_extend_to_i32(lhs, width);
    let rhs_signed = sign_extend_to_i32(rhs, width);
    
    let product = (lhs_signed as i64) * (rhs_signed as i64);
    
    let min_val = -(1i64 << (width * 8 - 1));
    let max_val = (1i64 << (width * 8 - 1)) - 1;
    
    let overflow = product < min_val || product > max_val;
    let result = (product as u32) & mask_for_width(width);
    
    (result, overflow, overflow)
}

fn check_add_overflow(lhs: u32, rhs: u32, _result: u32, width: u8, _signed: bool) -> bool {
    let lhs_signed = sign_extend_to_i32(lhs, width);
    let rhs_signed = sign_extend_to_i32(rhs, width);
    
    let sum = (lhs_signed as i64) + (rhs_signed as i64);
    let min_val = -(1i64 << (width * 8 - 1));
    let max_val = (1i64 << (width * 8 - 1)) - 1;
    
    sum < min_val || sum > max_val
}

fn check_sub_overflow(lhs: u32, rhs: u32, _result: u32, width: u8, _signed: bool) -> bool {
    let lhs_signed = sign_extend_to_i32(lhs, width);
    let rhs_signed = sign_extend_to_i32(rhs, width);
    
    let diff = (lhs_signed as i64) - (rhs_signed as i64);
    let min_val = -(1i64 << (width * 8 - 1));
    let max_val = (1i64 << (width * 8 - 1)) - 1;
    
    diff < min_val || diff > max_val
}
