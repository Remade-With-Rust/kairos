#![no_std]
//! Compile gate for the house crate `serde_json` under no_std on the Kairos targets.
extern crate alloc;
pub fn parse(s: &str) -> serde_json::Result<serde_json::Value> { serde_json::from_str(s) }
