mod akb;
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

#[derive(Debug, IntoEnumIterator)]
pub enum ResourceMagic {
    TLG0,
    TLG5,
    TLG6,
    PB3B,
    YCG,
    AKB,
    GYU,
    GYUUniversal,
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
            // TLG0.0\x00sds\x1a
            [84, 76, 71, 48, 46, 48, 0, 115, 100, 115, 26, ..] => Self::TLG0,
            // TLG5.0\x00raw\x1a
            [84, 76, 71, 53, 46, 48, 0, 114, 97, 119, 26, ..] => Self::TLG5,
            // TLG6.0\x00raw\x1a
            [84, 76, 71, 54, 46, 48, 0, 114, 97, 119, 26, ..] => Self::TLG6,
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
    pub fn is_universal(&self) -> bool {
        match self {
            Self::TLG0 => true,
            Self::TLG5 => true,
            Self::TLG6 => true,
            Self::PB3B => true,
            Self::YCG => true,
            Self::AKB => true,
            Self::GYU => false,
            Self::GYUUniversal => true,
            Self::Unrecognized => true,
        }
    }
    pub fn get_schemes(&self) -> Vec<Box<dyn ResourceScheme>> {
        match self {
            ResourceMagic::TLG0 => tlg::Tlg0Scheme::get_schemes(),
            ResourceMagic::TLG5 => tlg::Tlg5Scheme::get_schemes(),
            ResourceMagic::TLG6 => tlg::Tlg6Scheme::get_schemes(),
            ResourceMagic::PB3B => pb3b::Pb3bScheme::get_schemes(),
            ResourceMagic::YCG => ycg::YcgScheme::get_schemes(),
            ResourceMagic::AKB => akb::AkbScheme::get_schemes(),
            ResourceMagic::GYU => gyu::GyuScheme::get_schemes(),
            ResourceMagic::GYUUniversal => {
                vec![Box::new(gyu::GyuScheme::Universal)]
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
    RgbaImage { image: RgbaImage },
    Text(String),
    Other,
}
