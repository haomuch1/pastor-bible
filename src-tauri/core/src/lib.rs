//! The Pastor Bible: retrieval, citation verifier and generation.
//!
//! No GUI dependency lives here. The Tauri shell is one caller; the CLI harness
//! is another; the tests are a third.

pub mod api;
pub mod crisis;
pub mod index;
pub mod paths;
pub mod pipeline;
pub mod prompts;
pub mod retrieve;
pub mod sidecar;
pub mod tsk_abbrev;
pub mod verifier;
