#![deny(
    rust_2018_idioms,
    unreachable_pub,
    unsafe_code,
    unused_imports,
    unused_mut,
    missing_debug_implementations
)]

extern crate positioned_io_preview as positioned_io;

pub mod archive;
pub mod error;
pub mod magic;
pub mod resource;
pub mod scheme;
pub mod util;

use rust_embed::RustEmbed;

pub const ONE_MB: usize = 1 << 20;

#[derive(Debug, RustEmbed)]
#[folder = "resources/"]
pub struct Resources;
