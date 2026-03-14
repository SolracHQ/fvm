use serde::{Deserialize, Serialize};

use fvm_core::types::Word;

/// This is the Core of VM configuration, user will create ron config file and parse it into this struct, then pass it to VM when creating a new instance.
/// This config will define the memory size of the VM, the devices to be attached, the loacation of the initial rom image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfig {
    /// The main and must ram device, the vm needs it for device mapping, rom loading and stack.
    /// Must exist and be at least 16 MiB to be usable, but can be larger if desired.
    /// Must be a multiple of 4 KiB (4096 bytes) to align with the VM's page size.
    /// In case is not aligned, the VM will round up to the next multiple of 4 KiB and print a warning.
    #[serde(alias = "main_memory_size")]
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
                format!(
                    "failed to parse config as JSON ({json_error}) or RON ({ron_error})"
                )
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
        size: Word,
        #[serde(default)]
        general: GeneralDeviceConfig,
    },
    /// A simple device that reads/writes decimal integers from an input/output stream. Useful for testing and debugging.
    DecimalIo {
        /// Port-mapped I/O port for the device, must be unique across devices.
        port: Word,
        /// Input stream for the device. The device will read lines from this stream and parse them as decimal integers when the VM reads from the device.
        input: String,
        /// Output stream for the device. The device will write decimal integers to this stream when the VM writes to the device.
        output: String,
        #[serde(default)]
        general: GeneralDeviceConfig,
    },
    /// A byte-oriented device that reads and writes hexadecimal text, one byte per line.
    HexIo {
        /// Port-mapped I/O port for the device, must be unique across devices.
        port: Word,
        /// Input stream for the device. Each line is a hexadecimal byte such as `0x41` or `41`.
        input: String,
        /// Output stream for the device. Writes one hexadecimal byte per line.
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
        input: String,
        /// Output stream for the device. Bytes are written directly as-is.
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
        assert!(matches!(config.devices[0], DeviceConfig::HexIo { port: 0, .. }));
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

        assert!(matches!(config.devices[0], DeviceConfig::DecimalIo { port: 1, .. }));
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
}
