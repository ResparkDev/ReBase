#![allow(missing_docs)]

use anyhow::{Result, bail};
use std::path::PathBuf;

/// Options controlling how ReBase processes RON input and emits code.
///
/// Typical usage:
/// - Provide `input` pointing to a directory of `.ron` files (recursively scanned).
/// - Choose one or both outputs:
///   - `out_ue`: Directory where UE C++ code will be written.
///   - `out_rust_file`: Single Rust module file (e.g., `rebase_generated.rs`) to write.
/// - `namespace`: Optional C++ namespace chain (e.g., "Game::DB") applied outermost.
/// - `force`: Overwrite existing files.
/// - `dry_run`: Parse/validate only; do not write files.
#[derive(Debug, Clone)]
pub struct Options {
    /// Input directory containing `.ron` files (recursively scanned).
    pub input: PathBuf,

    /// Output directory for UE C++ code (header/source). None = skip UE generation.
    pub out_ue: Option<PathBuf>,

    /// Output path for a single generated Rust file (e.g., `rebase_generated.rs`).
    /// None = skip Rust generation.
    pub out_rust_file: Option<PathBuf>,

    /// Optional C++ namespace chain (outermost). Merged with any per-file namespace.
    pub namespace: Option<String>,

    /// Overwrite existing files without prompting.
    pub force: bool,

    /// Parse and validate, print plan, but do not write any files.
    pub dry_run: bool,
}

impl Options {
    /// Validate invariants for provided options.
    ///
    /// Ensures:
    /// - `input` exists and is a directory.
    /// - At least one of `out_ue` or `out_rust_file` is provided.
    pub fn validate(&self) -> Result<()> {
        if !self.input.exists() || !self.input.is_dir() {
            bail!(
                "Input path must be an existing directory: {}",
                self.input.display()
            );
        }

        if self.out_ue.is_none() && self.out_rust_file.is_none() {
            bail!("No outputs selected. Provide at least one of out_ue or out_rust_file");
        }

        Ok(())
    }
}
