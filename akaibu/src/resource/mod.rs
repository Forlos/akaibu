mod akb;
mod g00;
mod gyu;
mod jbp1;
mod pb3b;
mod tlg;
mod ycg;

use dyn_clone::DynClone;
use enum_iterator::IntoEnumIterator;
use image::RgbaImage;
use scroll::{Pread, LE};
use std::{fmt::Debug, path::PathBuf};
use tlg::TlgScheme;

#[derive(Debug, IntoEnumIterator)]
pub enum ResourceMagic {
    TLG,
    PB3B,
    YCG,
    AKB,
    GYU,
    GYUUniversal,
    G00,
    Unrecognized,
}

pub trait ResourceScheme: Debug + Send + Sync + DynClone {
    fn convert(&self, file_path: &PathBuf) -> anyhow::Result<ResourceType>;
    fn convert_from_bytes(
        &self,
        file_path: &PathBuf,
        buf: Vec<u8>,
    ) -> anyhow::Result<ResourceType>;
    fn get_name(&self) -> String;
    fn get_schemes() -> Vec<Box<dyn ResourceScheme>>
    where
        Self: Sized;
}

dyn_clone::clone_trait_object!(ResourceScheme);

impl ResourceMagic {
    pub fn parse_magic(buf: &[u8]) -> Self {
        match buf {
            // TLG
            [84, 76, 71, ..] => Self::TLG,
            // PB3B
            [80, 66, 51, 66, ..] => Self::PB3B,
            // YCG\x00
            [89, 67, 71, 0, ..] => Self::YCG,
            // AKB or AKB+
            [65, 75, 66, 32, ..] | [65, 75, 66, 43, ..] => Self::AKB,
            // GYU\x1a
            [71, 89, 85, 26, ..] => match buf.pread_with::<u32>(8, LE) {
                Ok(mt_seed) => {
                    if mt_seed == 0 {
                        Self::GYU
                    } else {
                        Self::GYUUniversal
                    }
                }
                Err(_) => Self::Unrecognized,
            },
            _ => Self::Unrecognized,
        }
    }
    pub fn parse_file_extension(file_path: &PathBuf) -> Self {
        match file_path.extension() {
            Some(extension) => match extension.to_str() {
                Some(extension) => match extension {
                    "g00" => Self::G00,
                    _ => Self::Unrecognized,
                },
                None => Self::Unrecognized,
            },
            None => Self::Unrecognized,
        }
    }
    pub fn is_universal(&self) -> bool {
        match self {
            Self::TLG => true,
            Self::PB3B => true,
            Self::YCG => true,
            Self::AKB => true,
            Self::GYU => false,
            Self::GYUUniversal => true,
            Self::G00 => true,
            Self::Unrecognized => true,
        }
    }
    pub fn get_schemes(&self) -> Vec<Box<dyn ResourceScheme>> {
        match self {
            ResourceMagic::TLG => TlgScheme::get_schemes(),
            ResourceMagic::PB3B => pb3b::Pb3bScheme::get_schemes(),
            ResourceMagic::YCG => ycg::YcgScheme::get_schemes(),
            ResourceMagic::AKB => akb::AkbScheme::get_schemes(),
            ResourceMagic::GYU => gyu::GyuScheme::get_schemes(),
            ResourceMagic::GYUUniversal => {
                vec![Box::new(gyu::GyuScheme::Universal)]
            }
            ResourceMagic::G00 => g00::G00Scheme::get_schemes(),
            ResourceMagic::Unrecognized => vec![],
        }
    }
    pub fn get_all_schemes() -> Vec<Box<dyn ResourceScheme>> {
        ResourceMagic::into_enum_iter()
            .map(|arc| arc.get_schemes())
            .flatten()
            .collect()
    }
}

#[derive(Debug, Clone)]
pub enum ResourceType {
    SpriteSheet { sprites: Vec<RgbaImage> },
    RgbaImage { image: RgbaImage },
    Text(String),
    Other,
}
