#![forbid(unsafe_code)]

//! Core semantic domain model for The Blob.
//!
//! This crate intentionally contains no solver, renderer, Nix, AI-model or
//! platform implementation. It exists to keep the semantic boundaries frozen
//! independently from prototype technologies.

pub mod events;
pub mod execution;
pub mod graphics;
pub mod history;
pub mod ids;
pub mod system;
pub mod world;

pub use events::*;
pub use execution::*;
pub use graphics::*;
pub use history::*;
pub use ids::*;
pub use system::*;
pub use world::*;
