use clap::Parser;
use std::fs;
use std::path::Path;

use fvm_vm::vm::config::{DeviceConfig, GeneralDeviceConfig, VmConfig};

#[derive(Parser)]
#[command(name = "fvm-vm")]
#[command(about = "Run FVM object files with the Rust VM")]
pub struct Args {
    /// Input object file (.fo)
    pub input: Option<String>,

    /// Full VM config as inline JSON/RON or as a path to a JSON/RON file.
    #[arg(long)]
    pub config: Option<String>,

    /// Main RAM size in bytes.
    #[arg(short = 'm', long)]
    pub memory_size: Option<u32>,

    /// Port number for the default decimal I/O device.
    #[arg(long)]
    pub decimal_port: Option<u32>,

    /// Input source for the decimal I/O device.
    #[arg(long)]
    pub decimal_input: Option<String>,

    /// Output sink for the decimal I/O device.
    #[arg(long)]
    pub decimal_output: Option<String>,

    /// Disable the default decimal I/O device entirely.
    #[arg(long)]
    pub no_decimal_io: bool,
}

impl Args {
    pub fn to_config(&self) -> Result<VmConfig, String> {
        if let Some(config_arg) = &self.config {
            let source = load_config_source(config_arg)?;
            let mut config = VmConfig::parse(&source)?;

            if let Some(input) = &self.input {
                config.rom = input.clone();
            }
            if let Some(memory_size) = self.memory_size {
                config.mem_size = memory_size;
            }

            return Ok(config);
        }

        let input = self
            .input
            .clone()
            .ok_or_else(|| "missing ROM input path: provide INPUT or --config".to_string())?;

        let devices = if self.no_decimal_io {
            Vec::new()
        } else {
            vec![DeviceConfig::DecimalIo {
                port: self.decimal_port.unwrap_or(0),
                input: self
                    .decimal_input
                    .clone()
                    .unwrap_or_else(|| "stdin".to_string()),
                output: self
                    .decimal_output
                    .clone()
                    .unwrap_or_else(|| "stdout".to_string()),
                general: GeneralDeviceConfig::default(),
            }]
        };

        Ok(VmConfig {
            mem_size: self.memory_size.unwrap_or(16 * 1024 * 1024),
            rom: input,
            devices,
        })
    }
}

fn load_config_source(config_arg: &str) -> Result<String, String> {
    let path = Path::new(config_arg);
    if path.exists() {
        return fs::read_to_string(path)
            .map_err(|error| format!("failed to read config file '{}': {error}", path.display()));
    }

    Ok(config_arg.to_string())
}
