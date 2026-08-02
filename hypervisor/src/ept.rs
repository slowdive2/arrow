//! intel extended page tables.
//!
//! this file is the public façade. the hardware encodings and backing table
//! layout live in small submodules so the mapping code can grow separately.

mod entry;
mod tables;
mod types;

pub use entry::*;
pub use tables::*;
pub use types::*;
