//! The Pastor Bible: retrieval, citation verifier and generation.
//!
//! No GUI dependency lives here. The Tauri shell is one caller; the CLI harness
//! is another; the tests are a third.

pub mod index;
pub mod retrieve;
pub mod tsk_abbrev;
pub mod verifier;
