mod cli;

use clap::Parser;
use fvm_vm::vm::VM;

fn main() {
    let args = cli::Args::parse();
    let config = match args.to_config() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }
    };

    match VM::new(config).and_then(|mut vm| vm.run()) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }
    }
}
