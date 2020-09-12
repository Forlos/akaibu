mod jbp1;
mod pb3b;

use crate::error::AkaibuError;
use image::RgbaImage;
use tlg_rs::formats::{tlg0::Tlg0, tlg6::Tlg6};

#[derive(Debug)]
pub enum ResourceMagic {
    TLG0,
    TLG5,
    TLG6,
    PB3B,
    Unrecognized,
}

impl ResourceMagic {
    pub fn parse_magic(buf: &[u8]) -> Self {
        match buf {
            // TLG0.0\x00sds\x1a
            [84, 76, 71, 48, 46, 48, 0, 115, 100, 115, 26, ..] => Self::TLG0,
            // TLG5.0\x00raw\x1a
            [84, 76, 71, 53, 46, 48, 0, 114, 97, 119, 26, ..] => Self::TLG5,
            // TLG6.0\x00raw\x1a
            [84, 76, 71, 54, 46, 48, 0, 114, 97, 119, 26, ..] => Self::TLG6,
            [80, 66, 51, 66, ..] => Self::PB3B,
            _ => Self::Unrecognized,
        }
    }
    pub fn parse(&self, buf: Vec<u8>) -> anyhow::Result<ResourceType> {
        match self {
            Self::TLG0 => {
                let image = Tlg0::from_bytes(&buf)?.to_rgba_image()?;
                Ok(ResourceType::RgbaImage { image })
            }
            Self::TLG5 => Err(AkaibuError::Unimplemented.into()),
            Self::TLG6 => {
                let image = Tlg6::from_bytes(&buf)?.to_rgba_image()?;
                Ok(ResourceType::RgbaImage { image })
            }
            Self::PB3B => {
                let pb3b = pb3b::Pb3b::from_bytes(buf)?;
                Ok(ResourceType::RgbaImage { image: pb3b.image })
            }
            Self::Unrecognized => Ok(ResourceType::Other),
        }
    }
}

#[derive(Debug)]
pub enum ResourceType {
    RgbaImage { image: RgbaImage },
    Text(String),
    Other,
}
