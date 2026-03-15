use serde::{Deserialize, Deserializer, Serialize};

use fvm_core::types::Word;

fn stdin() -> String {
    "stdin".to_string()
}

fn stdout() -> String {
    "stdout".to_string()
}

/// Parse human-readable memory sizes like "128mb", "2gb", "512kb", or raw bytes as u32.
fn parse_memory_size<'de, D>(deserializer: D) -> Result<Word, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct MemorySizeVisitor;

    impl<'de> Visitor<'de> for MemorySizeVisitor {
        type Value = Word;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str(
                "an integer or human-readable memory size like '128mb', '2gb', '512kb'",
            )
        }

        fn visit_u32<E>(self, value: u32) -> Result<Word, E>
        where
            E: de::Error,
        {
            Ok(value)
        }

        fn visit_u64<E>(self, value: u64) -> Result<Word, E>
        where
            E: de::Error,
        {
            if value > u32::MAX as u64 {
                Err(E::custom("memory size too large for u32"))
            } else {
                Ok(value as u32)
            }
        }

        fn visit_str<E>(self, value: &str) -> Result<Word, E>
        where
            E: de::Error,
        {
            parse_human_readable_size(value).map_err(E::custom)
        }

        fn visit_string<E>(self, value: String) -> Result<Word, E>
        where
            E: de::Error,
        {
            self.visit_str(&value)
        }
    }

    deserializer.deserialize_any(MemorySizeVisitor)
}

fn parse_human_readable_size(input: &str) -> Result<Word, String> {
    let input = input.trim().to_lowercase();

    let (number_str, suffix) = if let Some(pos) = input.find(|c: char| c.is_alphabetic()) {
        input.split_at(pos)
    } else {
        (input.as_str(), "")
    };

    let base: u64 = number_str
        .trim()
        .parse()
        .map_err(|_| format!("invalid number: '{}'", number_str))?;

    let multiplier: u64 = match suffix.trim() {
        "" => 1,
        "b" | "byte" | "bytes" => 1,
        "kb" | "k" => 1024,
        "mb" | "m" => 1024 * 1024,
        "gb" | "g" => 1024 * 1024 * 1024,
        "tb" | "t" => 1024 * 1024 * 1024 * 1024,
        _ => return Err(format!("unknown size suffix: '{}'", suffix)),
    };

    let result = base.saturating_mul(multiplier);
    if result > u32::MAX as u64 {
        return Err(format!(
            "memory size too large: {} (max 4GB)",
            base.to_string() + suffix
        ));
    }

    Ok(result as u32)
}

/// This is the Core of VM configuration, user will create ron config file and parse it into this struct, then pass it to VM when creating a new instance.
/// This config will define the memory size of the VM, the devices to be attached, the loacation of the initial rom image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfig {
    /// The main and must ram device, the vm needs it for device mapping, rom loading and stack.
    /// Must exist and be at least 16 MiB to be usable, but can be larger if desired.
    /// Must be a multiple of 4 KiB (4096 bytes) to align with the VM's page size.
    /// In case is not aligned, the VM will round up to the next multiple of 4 KiB and print a warning.
    /// Can be specified as a raw byte count or human-readable format like "128mb", "2gb".
    #[serde(alias = "main_memory_size", deserialize_with = "parse_memory_size")]
    pub mem_size: Word,
    /// Initial ROM image to load into memory at startup. This is typically the compiled output of the assembler, must follow the fvm binary format.
    /// The VM will load this into the main RAM device after the device mapping table.
    #[serde(alias = "rom_image")]
    pub rom: String,
    /// Additional devices to attach to the VM. The VM will create these devices and map them into the address space after the main RAM device.
    pub devices: Vec<DeviceConfig>,
}

impl VmConfig {
    pub fn parse(source: &str) -> Result<Self, String> {
        serde_json::from_str(source).or_else(|json_error| {
            ron::from_str(source).map_err(|ron_error| {
                format!("failed to parse config as JSON ({json_error}) or RON ({ron_error})")
            })
        })
    }
}

/// General configuration for devices, used by multiple device types.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralDeviceConfig {
    /// custom 8-byte ASCII identifier for deviece table on kernel, this overrides the device's internal id() method.
    /// This is useful for devices that want to have a fixed address in the device table, or for testing with mock devices.
    /// If None, the VM will use the device's internal id() method.
    pub id: Option<[u8; 8]>,
    /// If true, the VM will fail to start if there is not enough address space to map this device.
    /// Users can disable this check if the device is not critical or if it can be used via port-mapped I/O instead of memory-mapped I/O or viceversa in case ports are exhausted.
    #[serde(alias = "fail_if_not_enough_address_space")]
    pub fail_fast: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeviceConfig {
    /// For cases where a second RAM device is needed
    Ram {
        /// Size of the RAM device in bytes. Must be a multiple of 4 KiB (4096 bytes) to align with the VM's page size.
        /// Can be specified as a raw byte count or human-readable format like "128mb", "2gb".
        #[serde(deserialize_with = "parse_memory_size")]
        size: Word,
        #[serde(default)]
        general: GeneralDeviceConfig,
    },
    /// A simple device that reads/writes decimal integers from an input/output stream. Useful for testing and debugging.
    DecimalIo {
        /// Port-mapped I/O port for the device, must be unique across devices.
        port: Word,
        /// Input stream for the device. The device will read lines from this stream and parse them as decimal integers when the VM reads from the device.
        #[serde(default = "stdin")]
        input: String,
        /// Output stream for the device. The device will write decimal integers to this stream when the VM writes to the device.
        #[serde(default = "stdout")]
        output: String,
        #[serde(default)]
        general: GeneralDeviceConfig,
    },
    /// A byte-oriented device that reads and writes hexadecimal text, one byte per line.
    HexIo {
        /// Port-mapped I/O port for the device, must be unique across devices.
        port: Word,
        /// Input stream for the device. Each line is a hexadecimal byte such as `0x41` or `41`.
        #[serde(default = "stdin")]
        input: String,
        /// Output stream for the device. Writes one hexadecimal byte per line.
        #[serde(default = "stdout")]
        output: String,
        #[serde(default)]
        general: GeneralDeviceConfig,
    },
    /// A raw byte-oriented device that reads and writes bytes directly without formatting.
    /// Useful for programs that process raw binary data or ASCII text directly.
    RawIo {
        /// Port-mapped I/O port for the device, must be unique across devices.
        port: Word,
        /// Input stream for the device. Bytes are read directly as-is.
        #[serde(default = "stdin")]
        input: String,
        /// Output stream for the device. Bytes are written directly as-is.
        #[serde(default = "stdout")]
        output: String,
        #[serde(default)]
        general: GeneralDeviceConfig,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_config_with_hex_device() {
        let config = VmConfig::parse(
            r#"{
                "mem_size": 16777216,
                "rom": "program.fo",
                "devices": [
                    {
                        "type": "hex_io",
                        "port": 0,
                        "input": "stdin",
                        "output": "stdout",
                        "general": {}
                    }
                ]
            }"#,
        )
        .expect("config should parse");

        assert_eq!(config.mem_size, 16 * 1024 * 1024);
        assert!(matches!(
            config.devices[0],
            DeviceConfig::HexIo { port: 0, .. }
        ));
    }

    #[test]
    fn parses_ron_config_with_decimal_device() {
        let config = VmConfig::parse(
            r#"(
                mem_size: 16777216,
                rom: "program.fo",
                devices: [
                    (
                        type: "decimal_io",
                        port: 1,
                        input: "stdin",
                        output: "stdout",
                    ),
                ],
            )"#,
        )
        .expect("config should parse");

        assert!(matches!(
            config.devices[0],
            DeviceConfig::DecimalIo { port: 1, .. }
        ));
    }

    #[test]
    fn parses_legacy_json_field_names() {
        let config = VmConfig::parse(
            r#"{
                "main_memory_size": 16777216,
                "rom_image": "legacy.fo",
                "devices": []
            }"#,
        )
        .expect("legacy config should parse");

        assert_eq!(config.mem_size, 16 * 1024 * 1024);
        assert_eq!(config.rom, "legacy.fo");
    }

    #[test]
    fn parses_human_readable_memory_sizes_ron() {
        let tests = vec![
            (r#"(mem_size: "128mb", rom: "test.fo", devices: [])"#, 128 * 1024 * 1024),
            (r#"(mem_size: "2gb", rom: "test.fo", devices: [])"#, 2 * 1024 * 1024 * 1024),
            (r#"(mem_size: "512kb", rom: "test.fo", devices: [])"#, 512 * 1024),
            (r#"(mem_size: "256mb", rom: "test.fo", devices: [])"#, 256 * 1024 * 1024),
        ];

        for (ron_str, expected) in tests {
            let config = VmConfig::parse(ron_str).expect("config should parse");
            assert_eq!(config.mem_size, expected);
        }
    }

    #[test]
    fn parses_human_readable_memory_sizes_with_variants() {
        let tests = vec![
            (r#"(mem_size: "128 mb", rom: "test.fo", devices: [])"#, 128 * 1024 * 1024),
            (r#"(mem_size: "2 GB", rom: "test.fo", devices: [])"#, 2 * 1024 * 1024 * 1024),
            (r#"(mem_size: "512 KB", rom: "test.fo", devices: [])"#, 512 * 1024),
        ];

        for (ron_str, expected) in tests {
            let config = VmConfig::parse(ron_str).expect("config should parse");
            assert_eq!(config.mem_size, expected);
        }
    }

    #[test]
    fn device_ram_supports_human_readable_size() {
        let config = VmConfig::parse(
            r#"(
                mem_size: "16mb",
                rom: "test.fo",
                devices: [
                    (
                        type: "ram",
                        size: "256mb",
                    ),
                ],
            )"#,
        )
        .expect("config should parse");

        assert_eq!(config.mem_size, 16 * 1024 * 1024);
        assert!(matches!(
            config.devices[0],
            DeviceConfig::Ram { size: 268435456, .. } // 256mb
        ));
    }
}
