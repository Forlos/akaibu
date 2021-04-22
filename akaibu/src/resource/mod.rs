mod akb;
mod common;
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
use std::fmt::Debug;
use std::path::Path;
use tlg::TlgScheme;

#[derive(Debug, IntoEnumIterator)]
pub enum ResourceMagic {
    Tlg,
    Pb3b,
    Ycg,
    Akb,
    Gyu,
    GyuUniversal,
    G00,
    Png,
    Jpg,
    Bmp,
    Ico,
    Riff,
    Unrecognized,
}

pub trait ResourceScheme: Debug + Send + Sync + DynClone {
    fn convert(&self, file_path: &Path) -> anyhow::Result<ResourceType>;
    fn convert_from_bytes(
        &self,
        file_path: &Path,
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
            [84, 76, 71, ..] => Self::Tlg,
            // PB3B
            [80, 66, 51, 66, ..] => Self::Pb3b,
            // YCG\x00
            [89, 67, 71, 0, ..] => Self::Ycg,
            // AKB or AKB+
            [65, 75, 66, 32, ..] | [65, 75, 66, 43, ..] => Self::Akb,
            // GYU\x1a
            [71, 89, 85, 26, ..] => match buf.pread_with::<u32>(8, LE) {
                Ok(mt_seed) => {
                    if mt_seed == 0 {
                        Self::Gyu
                    } else {
                        Self::GyuUniversal
                    }
                }
                Err(_) => Self::Unrecognized,
            },
            [137, 80, 78, 71, 13, 10, 26, 10, ..]
            | [135, 80, 78, 71, 13, 10, 26, 10, ..] => Self::Png,
            [255, 216, 255, ..] => Self::Jpg,
            [66, 77, ..] => Self::Bmp,
            [0, 0, 1, 0, ..] => Self::Ico,
            [82, 73, 70, 70, ..] => Self::Riff,
            _ => Self::Unrecognized,
        }
    }
    pub fn parse_file_extension(file_path: &Path) -> Self {
        match file_path.extension() {
            Some(extension) => match extension.to_str() {
                Some(extension) => match extension {
                    "g00" => Self::G00,
                    "png" => Self::Png,
                    _ => Self::Unrecognized,
                },
                None => Self::Unrecognized,
            },
            None => Self::Unrecognized,
        }
    }
    pub fn is_universal(&self) -> bool {
        match self {
            Self::Tlg => true,
            Self::Pb3b => true,
            Self::Ycg => true,
            Self::Akb => true,
            Self::Gyu => false,
            Self::GyuUniversal => true,
            Self::G00 => true,
            Self::Png => true,
            Self::Jpg => true,
            Self::Bmp => true,
            Self::Ico => true,
            Self::Riff => true,
            Self::Unrecognized => true,
        }
    }
    pub fn get_schemes(&self) -> Vec<Box<dyn ResourceScheme>> {
        match self {
            ResourceMagic::Tlg => TlgScheme::get_schemes(),
            ResourceMagic::Pb3b => pb3b::Pb3bScheme::get_schemes(),
            ResourceMagic::Ycg => ycg::YcgScheme::get_schemes(),
            ResourceMagic::Akb => akb::AkbScheme::get_schemes(),
            ResourceMagic::Gyu => gyu::GyuScheme::get_schemes(),
            ResourceMagic::GyuUniversal => {
                vec![Box::new(gyu::GyuScheme::Universal)]
            }
            ResourceMagic::G00 => g00::G00Scheme::get_schemes(),
            Self::Png | Self::Jpg | Self::Bmp | Self::Ico | Self::Riff => {
                vec![Box::new(common::Common)]
            }
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
