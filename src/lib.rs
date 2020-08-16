#![deny(
    rust_2018_idioms,
    unreachable_pub,
    unsafe_code,
    unused_imports,
    missing_debug_implementations
)]

pub mod archive;
pub mod error;
pub mod magic;
pub mod scheme;
pub mod util;

use rust_embed::RustEmbed;

#[derive(Debug, RustEmbed)]
#[folder = "resources/"]
pub struct Resources;
