/*!
ReBase library facade

This crate exposes a simple facade over the internal modules. The heavy lifting
is implemented inside `model`, `parse`, `ue`, and `rustgen`. Most users only
need `Options` and `run(...)`.
*/

pub mod model;
pub mod options;
pub mod parse;
pub mod rustgen;
pub mod ue;

// Re-exports for the common entry points
pub use options::Options;
pub use parse::run;
