mod args;
mod assembler;
mod error;

use args::Args;
use clap::Parser;

fn main() {
    let args = Args::parse();

    let output_path = args.output.unwrap_or_else(|| {
        let mut path = args.input.clone();
        path.push_str(".fo");
        path
    });

    if let Err(e) = assemble_command(&args.input, &output_path) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn assemble_command(input: &str, output: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Assembling {} -> {}", input, output);
    let format = assembler::assemble_file(input)?;
    let bytes = format.to_bytes()?;
    std::fs::write(output, bytes)?;
    println!("OK");
    Ok(())
}
