use clap::Parser;

#[derive(Parser)]
#[command(name = "fvm-assembler")]
#[command(about = "FVM Assembly compiler", long_about = None)]
pub struct Args {
    /// Input assembly file (.fa)
    pub input: String,

    /// Output object file (.fo)
    #[arg(short, long)]
    pub output: Option<String>,
}
