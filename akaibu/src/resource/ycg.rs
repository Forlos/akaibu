use crate::{error::AkaibuError, util::zlib_decompress};
use anyhow::Context;
use image::{buffer::ConvertBuffer, ImageBuffer};
use scroll::{Pread, LE};
use std::{fs::File, io::Read, path::Path};

use super::{ResourceScheme, ResourceType};

#[derive(Debug, Clone)]
pub(crate) enum YcgScheme {
    Universal,
}

impl ResourceScheme for YcgScheme {
    fn convert(&self, file_path: &Path) -> anyhow::Result<ResourceType> {
        let mut buf = Vec::with_capacity(1 << 20);
        let mut file = File::open(file_path)?;
        file.read_to_end(&mut buf)?;
        self.from_bytes(buf)
    }
    fn convert_from_bytes(
        &self,
        _file_path: &Path,
        buf: Vec<u8>,
    ) -> anyhow::Result<ResourceType> {
        self.from_bytes(buf)
    }

    fn get_name(&self) -> String {
        format!(
            "[YCG] {}",
            match self {
                Self::Universal => "Universal",
            }
        )
    }

    fn get_schemes() -> Vec<Box<dyn ResourceScheme>>
    where
        Self: Sized,
    {
        vec![Box::new(Self::Universal)]
    }
}

impl YcgScheme {
    fn from_bytes(&self, buf: Vec<u8>) -> anyhow::Result<ResourceType> {
        let width = buf.pread_with::<u32>(4, LE)?;
        let height = buf.pread_with::<u32>(8, LE)?;
        let version = buf.pread_with::<u32>(16, LE)?;
        let size = buf.pread_with::<u32>(32, LE)? as usize;
        let compressed_size = buf.pread_with::<u32>(36, LE)? as usize;
        match version {
            1 => {
                let mut result = zlib_decompress(
                    &buf.get(0x38..).context("Out of bounds access")?,
                )?
                .get(..size)
                .context("Out of bounds access")?
                .to_vec();
                result.extend(zlib_decompress(
                    &buf.get(0x38 + compressed_size..)
                        .context("Out of bounds access")?,
                )?);
                let image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
                    ImageBuffer::from_vec(width, height, result)
                        .context("Invalid image resolution")?;
                Ok(ResourceType::RgbaImage {
                    image: image.convert(),
                })
            }
            _ => Err(AkaibuError::Unimplemented(format!(
                "Unsupported YCG version {}",
                version,
            ))
            .into()),
        }
    }
}
