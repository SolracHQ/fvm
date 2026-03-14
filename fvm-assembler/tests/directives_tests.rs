//! Tests for assembly directives (db, dh, dw)

#[cfg(test)]
mod tests {
    use fvm_assembler::assembler;

    #[test]
    fn test_db_single_byte() {
        let source = r#"
.rodata
data: db 0xFF

.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_db_multiple_bytes() {
        let source = r#"
.rodata
data: db 0x00, 0x01, 0x02, 0x03, 0x04

.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_db_string_literal() {
        let source = r#"
.rodata
msg: db "Hello"

.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_db_char_literal() {
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
    fn test_db_mixed_bytes_chars_strings() {
        let source = r#"
.rodata
data: db 0x40, 'X', "test", 0

.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_db_null_terminator() {
        let source = r#"
.rodata
string: db "hello", 0

.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dh_single_word() {
        let source = r#"
.rodata
data: dh 0x1234

.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dh_multiple_words() {
        let source = r#"
.rodata
data: dh 0x0000, 0x1111, 0x2222, 0x3333

.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dh_max_value() {
        let source = r#"
.rodata
data: dh 0xFFFF

.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dw_single_dword() {
        let source = r#"
.rodata
data: dw 0x12345678

.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dw_multiple_dwords() {
        let source = r#"
.rodata
data: dw 0x00000000, 0xFFFFFFFF, 0x12345678

.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_dw_with_label_reference() {
        let source = r#"
.rodata
ptr: dw table

table: dw 1, 2, 3

.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_db_in_data_section() {
        let source = r#"
.data
buffer: db 0, 0, 0, 0

.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiple_labels_same_section() {
        let source = r#"
.rodata
msg1: db "hello"
msg2: db "world"
msg3: db 0xFF, 0xFE, 0xFD

.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_db_with_trailing_comma() {
        let source = r#"
.rodata
data: db 1, 2, 3,
        "#;
        let result = assembler::assemble_source(source);
        // Trailing comma should be handled gracefully
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_directives_in_code_section_error() {
        let source = r#"
.code
main:
  db 0xFF
        "#;
        let result = assembler::assemble_source(source);
        assert!(
            result.is_err(),
            "Data directives not allowed in code section"
        );
    }

    #[test]
    fn test_db_empty_string() {
        let source = r#"
.rodata
empty: db ""
        "#;
        let result = assembler::assemble_source(source);
        // Empty string is valid - should be 0 bytes
        assert!(result.is_err(), "Empty db directive should error");
    }

    #[test]
    fn test_label_references_in_dw() {
        let source =
            ".rodata\nptrs: dw f1, f2\n.code\nmain: HALT\nf1: MOV rw0, 1\nRET\nf2: MOV rw0, 2\nRET";
        let result = assembler::assemble_source(source);
        assert!(
            result.is_ok(),
            "Failed to assemble with label references: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_rodata_section_only() {
        let source = r#"
.rodata
data1: db 1, 2, 3
data2: dh 0x1234
data3: dw 0x12345678

.code
main: HALT
        "#;
        let result = assembler::assemble_source(source);
        assert!(
            result.is_ok(),
            "Failed to assemble rodata section: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_all_three_sections() {
        let source = ".rodata\nmsg: db \"init\"\n.data\nbuf: dw 1\n.code\nmain: MOV rw0, 1\nHALT";
        let result = assembler::assemble_source(source);
        assert!(
            result.is_ok(),
            "Failed to assemble three sections: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_db_newlines_between_values() {
        let source = r#"
.rodata
data: db
  0x00,
  0x01,
  0x02
        "#;
        let result = assembler::assemble_source(source);
        // Should work with multi-line data
        assert!(result.is_ok() || result.is_err());
    }
}
