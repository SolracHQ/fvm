//! Integration tests for full assembly process

#[cfg(test)]
mod tests {
    use fvm_assembler::assembler;
    use fvm_core::section::Section;

    #[test]
    fn test_assemble_nop_halt() {
        let source = "main: NOP\nHALT";
        let result = assembler::assemble_source(source);
        assert!(result.is_ok(), "Failed to assemble: {:?}", result.err());
        let format = result.unwrap();
        let bytes = format.to_bytes();
        assert!(bytes.is_ok());
    }

    #[test]
    fn test_assemble_simple_mov() {
        let source = "main: MOV rw0, 42\nHALT";
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
        assert!(result.unwrap().to_bytes().is_ok());
    }

    #[test]
    fn test_assemble_arithmetic() {
        let source = r#"
main:
MOV rw0, 10
MOV rw1, 3
ADD rw0, rw1
HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
        assert!(result.unwrap().to_bytes().is_ok());
    }

    #[test]
    fn test_assemble_with_label() {
        let source = r#"
main:
  MOV rw0, 42
  JMP main
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_rodata_section() {
        let source = r#"
.rodata
msg: db "Hello", 0

.code
main:
  MOV rw0, msg
  HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_data_section() {
        let source = r#"
.data
counter: dw 0

.code
main:
  MOV rw0, 10
  HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_db_directive() {
        let source = r#"
.rodata
data: db 1, 2, 3, 4, 5
.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_db_with_string() {
        let source = r#"
.rodata
msg: db "test"
.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_dh_directive() {
        let source = r#"
.rodata
data: dh 0x1234, 0x5678
.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_dw_directive() {
        let source = r#"
.rodata
data: dw 0x12345678, 0xABCDEF00
.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_complex_program() {
        let source = r#"
.rodata
msg: db "Hello, world!\n", 0

.code
main:
  MOV rw1, msg
loop:
  LOAD rb2, rw1
  CMP rb2, 0
  JZ done
  OUT 0, rb2
  ADD rw1, 1
  JMP loop
done:
  HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_all_register_variants() {
        let source = r#"
main:
MOV rw0, 0
MOV rw1, 0
MOV rw2, 0
MOV rw3, 0
MOV rw4, 0
MOV rw5, 0
MOV rw6, 0
MOV rw7, 0
MOV rw8, 0
MOV rw9, 0
MOV rw10, 0
MOV rw11, 0
MOV rw12, 0
MOV rw13, 0
MOV rw14, 0
MOV rw15, 0
HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_byte_lane_operations() {
        let source = r#"
main:
MOV rb0, 0xFF
MOV rh0, 0x00
ADD rb0, 0x01
HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_stack_operations() {
        let source = r#"
main:
PUSH rw0
PUSH rw1
POP rw2
POP rw3
HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_conditional_jumps() {
        let source = r#"
main:
MOV rw0, 5
MOV rw1, 3
CMP rw0, rw1
JZ equal
JMP not_equal
equal:
  HALT
not_equal:
  HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_call_ret() {
        let source = r#"
main:
CALL func
HALT

func:
  MOV rw0, 42
  RET
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_bitwise_operations() {
        let source = r#"
main:
MOV rw0, 0xFF00
MOV rw1, 0x0F0F
AND rw0, rw1
OR rw0, rw1
XOR rw0, rw1
NOT rw0
HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_undefined_label_error() {
        let source = "JMP undefined_label";
        let result = assembler::assemble_source(source);
        assert!(result.is_err(), "Should error on undefined label");
    }

    #[test]
    fn test_assemble_multiple_sections() {
        let source = r#"
.rodata
data1: db 0xFF

.data
data2: dw 0x12345678

.code
main:
  MOV rw0, 0
  HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_local_labels() {
        let source = r#"
main:
  MOV rw0, 0
  .loop:
    ADD rw0, 1
    CMP rw0, 10
    JNZ .loop
  HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(
            result.is_ok(),
            "Failed to assemble local labels: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_assemble_char_literal() {
        let source = r#"
.rodata
data: db 'A', 'B', 'C'
.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_zext_sext_instructions() {
        let source = r#"
main:
MOV rb0, 0x05
ZEXT rw0, rb0
MOV rh1, 0xFF
SEXT rw1, rh1
HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_sp_register() {
        let source = r#"
main:
MOV rw0, sp
PUSH rw0
POP rw1
HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_load_store() {
        let source = r#"
main:
MOV rw0, 0x1000
MOV rw1, 0xFF00
STORE rw0, rw1
LOAD rw2, rw0
HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_assemble_v3_header_layout() {
        let source = "main: HALT";
        let format = assembler::assemble_source(source).expect("assembly should succeed");
        let bytes = format.to_bytes().expect("serialization should succeed");

        assert_eq!(bytes[0], 0x46);
        assert_eq!(bytes[1], 0x56);
        assert_eq!(bytes[2], 0x4D);
        assert_eq!(bytes[3], 0x21);
        assert_eq!(bytes[4], 3);
        assert_eq!(&bytes[9..13], &(0u32).to_be_bytes());
        assert_eq!(&bytes[13..17], &(1u32).to_be_bytes());
        assert_eq!(&bytes[17..21], &(0u32).to_be_bytes());
        assert_eq!(&bytes[21..25], &(0u32).to_be_bytes());
        assert_eq!(
            bytes.len(),
            26,
            "25-byte header plus 1-byte HALT payload expected"
        );
    }

    #[test]
    fn test_assemble_section_aware_relocations() {
        let source = r#"
.rodata
code_ptr: dw main

.code
main:
  MOV rw0, code_ptr
  HALT

.data
data_ptr: dw main
        "#;
        let format = assembler::assemble_source(source).expect("assembly should succeed");

        assert_eq!(
            format.relocations,
            vec![(Section::RoData, 0), (Section::Code, 2), (Section::Data, 0),]
        );

        let bytes = format.to_bytes().expect("serialization should succeed");
        let relocation_start = 25 + format.ro_data.len() + format.code.len() + format.rw_data.len();
        assert_eq!(
            &bytes[relocation_start..relocation_start + 5],
            &[0u8, 0, 0, 0, 0]
        );
        assert_eq!(
            &bytes[relocation_start + 5..relocation_start + 10],
            &[1u8, 0, 0, 0, 2]
        );
        assert_eq!(
            &bytes[relocation_start + 10..relocation_start + 15],
            &[2u8, 0, 0, 0, 0]
        );
    }

    #[test]
    fn test_assemble_new_opcode_families() {
        let source = r#"
main:
  SHL rw0, rb1
  SHR rw0, 3
  SAR rh0, rb2
  ROL rb0, 1
  ROR rw1, rb3
  TUR rw2, rw3
  TKR rw4, rw5
  MMAP rw6, rw7, rw8
  MMAP rw6, rw7, 4096
  MUNMAP rw9, rw10
  MUNMAP rw9, 4096
  MPROTECT rw1, rw2, rb3
  HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(
            result.is_ok(),
            "Failed to assemble new opcode families: {:?}",
            result.err()
        );
    }
}
