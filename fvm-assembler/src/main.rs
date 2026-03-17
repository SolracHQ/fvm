mod args;

use args::Args;
use clap::Parser;
use fvm_assembler::assembler;

fn main() {
    let args = Args::parse();

    let output_path = args.output.unwrap_or_else(|| {
        let mut path = args.input.clone();
        path.push_str(".fo");
        path
    });

    if let Err(e) = assemble_command(&args.input, &output_path) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn assemble_command(input: &str, output: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Assembling {} -> {}", input, output);
    let artifacts = assembler::assemble_file(input)
        .map_err(|error| std::io::Error::other(assembler::diagnostic::render_error(&error)))?;
    let bytes = artifacts.format.to_bytes()?;
    std::fs::write(output, bytes)?;
    println!("OK");
    Ok(())
}
