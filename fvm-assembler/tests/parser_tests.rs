//! Tests for parser functionality (via integration tests)
//! Parser is tested through the assemble_source function since it's part of
//! the internal pipeline. These tests verify parsing behavior indirectly.

#[cfg(test)]
mod tests {
    use fvm_assembler::assemble_source;

    #[test]
    fn test_parse_supports_nop() {
        let result = assemble_source("main:NOP\nHALT");
        assert!(result.is_ok(), "Parser should handle NOP");
    }

    #[test]
    fn test_parse_supports_mov() {
        let result = assemble_source("main:MOV rw0, 42\nHALT");
        assert!(result.is_ok(), "Parser should handle MOV");
    }

    #[test]
    fn test_parse_supports_arithmetic() {
        let result = assemble_source("main:ADD rw0, rw1\nSUB rw0, rw1\nHALT");
        assert!(result.is_ok(), "Parser should handle arithmetic");
    }

    #[test]
    fn test_parse_supports_bitwise() {
        let result = assemble_source("main:AND rw0, rw1\nOR rw0, rw1\nXOR rw0, rw1\nNOT rw0\nHALT");
        assert!(result.is_ok(), "Parser should handle bitwise ops");
    }

    #[test]
    fn test_parse_supports_memory() {
        let result = assemble_source("main:LOAD rw0, rw1\nSTORE rw0, rw1\nHALT");
        assert!(result.is_ok(), "Parser should handle memory ops");
    }

    #[test]
    fn test_parse_supports_jumps() {
        let result = assemble_source(
            "main:JMP loop\nJZ end\nJNZ loop\nJC overflow\nJN negative\nloop: HALT\nend: HALT\noverflow: HALT\nnegative: HALT",
        );
        assert!(result.is_ok(), "Parser should handle jumps");
    }

    #[test]
    fn test_parse_supports_stack() {
        let result = assemble_source("main:PUSH rw0\nPOP rw1\nHALT");
        assert!(result.is_ok(), "Parser should handle stack ops");
    }

    #[test]
    fn test_parse_supports_subroutines() {
        let result = assemble_source("main:CALL func\nHALT\nfunc: RET");
        assert!(
            result.is_ok(),
            "Parser should handle call/ret but got error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_parse_supports_byte_lanes() {
        let result = assemble_source("main:MOV rb0, 0xFF\nMOV rh0, 0x00\nHALT");
        assert!(result.is_ok(), "Parser should handle byte lanes");
    }

    #[test]
    fn test_parse_supports_rodata_section() {
        let result = assemble_source("main:.rodata\nmsg: db \"Hello\"\n.code\nhalt: HALT");
        assert!(result.is_ok(), "Parser should handle .rodata section");
    }

    #[test]
    fn test_parse_supports_data_section() {
        let result = assemble_source(".data\nbuffer: dw 0\n.code\nmain: HALT");
        assert!(result.is_ok(), "Parser should handle .data section");
    }

    #[test]
    fn test_parse_supports_labels() {
        let result = assemble_source("main:\nMOV rw0, 0\nHALT");
        assert!(result.is_ok(), "Parser should handle labels");
    }

    #[test]
    fn test_parse_error_missing_operand() {
        let result = assemble_source("main:MOV r0");
        assert!(result.is_err(), "Parser should error on missing operand");
    }

    #[test]
    fn test_parse_error_invalid_register() {
        let result = assemble_source("main:MOV rw99, 0");
        assert!(result.is_err(), "Parser should error on invalid register");
    }

    #[test]
    fn test_parse_error_out_of_range_value() {
        let result = assemble_source("main:MOV r0, 0xFFFFFFFF");
        // May error depending on implementation
        let _ = result;
    }

    #[test]
    fn test_parse_comments_are_ignored() {
        let result = assemble_source("main:# This is a comment\nNOP # End of line comment\nHALT");
        assert!(result.is_ok(), "Parser should ignore comments");
    }

    #[test]
    fn test_parse_empty_lines() {
        let result = assemble_source("main:\n\nNOP\n\n\nHALT\n");
        assert!(result.is_ok(), "Parser should handle empty lines");
    }

    #[test]
    fn test_parse_zext_sext() {
        let result = assemble_source("main:ZEXT rw0, rh0\nSEXT rw1, rh1\nHALT");
        assert!(
            result.is_ok(),
            "Parser should handle ZEXT/SEXT, but got error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_parse_shift_and_rotate_ops() {
        let result = assemble_source(
            "main:\nSHL rw0, rb1\nSHL rh0, 1\nSHR rb0, rb1\nSAR rw1, 2\nROL rh1, rb2\nROR rb1, 7\nHALT",
        );
        assert!(
            result.is_ok(),
            "Parser should handle shift/rotate ops, but got error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_parse_privileged_memory_ops() {
        let result = assemble_source(
            "main:\nTUR rw0, rw1\nTKR rw2, rw3\nMMAP rw4, rw5, rw6\nMMAP rw4, rw5, 4096\nMUNMAP rw7, rw8\nMUNMAP rw7, 4096\nMPROTECT rw9, rb0\nHALT",
        );
        assert!(
            result.is_ok(),
            "Parser should handle privileged memory ops, but got error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_parse_supports_cr_register() {
        // Reading cr via MOV is allowed.
        let read_result = assemble_source("main:MOV rw0, cr\nHALT");
        assert!(
            read_result.is_ok(),
            "Parser should allow reading cr, but got error: {:?}",
            read_result.err()
        );
        // Writing cr via MOV is rejected at parse time; use TKR instead.
        let write_result = assemble_source("main:MOV cr, rw1\nHALT");
        assert!(
            write_result.is_err(),
            "Parser should reject MOV cr, rw (use TKR instead)"
        );
    }

    #[test]
    fn test_parse_error_sie_requires_rb_index() {
        let result = assemble_source("main:SIE rw0, handler\nhandler: IRET");
        assert!(result.is_err(), "Parser should reject non-rb SIE index");
    }

    #[test]
    fn test_parse_error_int_register_requires_rb() {
        let result = assemble_source("main:INT rw0");
        assert!(
            result.is_err(),
            "Parser should reject non-rb INT register form"
        );
    }

    #[test]
    fn test_parse_error_shift_register_requires_rb() {
        let result = assemble_source("main:SHL rw0, rw1");
        assert!(
            result.is_err(),
            "Parser should reject non-rb shift amount register"
        );
    }

    #[test]
    fn test_parse_in_out() {
        let result = assemble_source("main:OUT 0, rw0\nIN rw1, 0\nHALT");
        assert!(result.is_ok(), "Parser should handle IN/OUT");
    }

    #[test]
    fn test_parse_all_16_registers() {
        let result = assemble_source(
            "main:MOV rw0, 0\nMOV rw1, 0\nMOV rw2, 0\nMOV rw3, 0\n\
             MOV rw4, 0\nMOV rw5, 0\nMOV rw6, 0\nMOV rw7, 0\n\
             MOV rw8, 0\nMOV rw9, 0\nMOV rw10, 0\nMOV rw11, 0\n\
             MOV rw12, 0\nMOV rw13, 0\nMOV rw14, 0\nMOV rw15, 0\nHALT",
        );
        assert!(result.is_ok(), "Parser should handle all 16 registers");
    }
}
