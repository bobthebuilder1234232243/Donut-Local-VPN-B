//! Test harness for the parts of Donut Local VPN that do not need Tauri.
//!
//! The modules below are the real application sources, pulled in with `#[path]`
//! so the harness compiles exactly the code that ships.

#![allow(dead_code)]

#[path = "../../src/catalog.rs"]
pub mod catalog;

#[path = "../../src/engine/mod.rs"]
pub mod engine;

#[path = "../../src/log.rs"]
pub mod log;

#[path = "../../src/probe.rs"]
pub mod probe;

#[path = "../../src/settings.rs"]
pub mod settings;
