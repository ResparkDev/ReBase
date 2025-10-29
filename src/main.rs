use std::path::PathBuf;

use clap::Parser;

/// ReBase CLI
///
/// Generate code from RON schemas into:
/// - Unreal Engine 5 C++ headers/sources
/// - A single Rust source file you can include with include!(...)
///
/// Examples:
///   rebase --input ./Data --out-ue ./UE/Source/MyGame/ReBase --namespace Game::DB
///   rebase --input ./Data --out-rust-file ./target/rebase_generated.rs --namespace Game::DB
///   rebase --input ./Data --out-ue ./UE/Source/MyGame/ReBase --out-rust-file ./target/rebase_generated.rs --namespace Game::DB
#[derive(Parser, Debug)]
#[command(
    name = "rebase",
    version,
    about = "Generate UE5 C++ and a single Rust module from RON datasets",
    long_about = "Generate code from RON schemas into:\n- UE5 C++ headers/sources\n- Single-file Rust module for server usage\n\nBackends:\n  --out-ue <DIR>            Output directory for UE C++ headers/sources\n  --out-rust-file <FILE>    Output path for the single Rust module\n\nNotes:\n  - Outputs are overwritten on each run (no prompt).\n  - Use --dry-run to validate without writing files.\n\nExamples:\n  rebase --input ./Data --out-ue ./UE/Source/MyGame/ReBase --namespace Game::DB\n  rebase --input ./Data --out-rust-file ./target/rebase_generated.rs --namespace Game::DB\n  rebase --input ./Data --out-ue ./UE/Source/MyGame/ReBase --out-rust-file ./target/rebase_generated.rs --namespace Game::DB"
)]

struct Cli {
    /// Input directory containing .ron files (recursively)
    #[arg(short, long)]
    input: PathBuf,

    /// Output directory for UE C++ code (optional), e.g. ./UE/Source/MyGame/ReBase

    #[arg(long = "out-ue")]
    out_ue: Option<PathBuf>,

    /// Output path for single-file Rust module (optional), e.g. ./target/rebase_generated.rs
    #[arg(long = "out-rust-file")]
    out_rust_file: Option<PathBuf>,

    /// Optional C++ namespace chain, e.g. "Game::DB"
    #[arg(long)]
    namespace: Option<String>,

    /// Overwrite existing files without prompting (currently always overwritten)
    #[arg(long)]
    force: bool,

    /// Dry-run: parse/validate and print plan; do not write any files
    #[arg(long)]
    dry_run: bool,
}

fn main() {
    let cli = Cli::parse();

    let opts = rebase::Options {
        input: cli.input,

        out_ue: cli.out_ue,

        out_rust_file: cli.out_rust_file,

        namespace: cli.namespace,

        force: cli.force,

        dry_run: cli.dry_run,
    };

    match rebase::run(&opts) {
        Ok(units) => {
            if opts.dry_run {
                println!("Dry run successful.");
            }
            if units.is_empty() {
                println!("No units generated.");
            } else {
                println!("Generated {} unit(s):", units.len());
                for u in units {
                    println!(" - {}", u);
                }
            }
        }
        Err(err) => {
            eprintln!("Error: {:#}", err);
            std::process::exit(1);
        }
    }
}
