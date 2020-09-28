use anyhow::Context;
use image::buffer::ConvertBuffer;
use image::ImageBuffer;
use image::RgbaImage;
use scroll::Pread;
use scroll::LE;

use crate::error::AkaibuError;
use crate::util::zlib_decompress;

#[derive(Debug)]
pub(crate) struct Ycg {
    pub(crate) image: RgbaImage,
}

impl Ycg {
    pub(crate) fn from_bytes(buf: Vec<u8>) -> anyhow::Result<Self> {
        let width = buf.pread_with::<u32>(4, LE)?;
        let height = buf.pread_with::<u32>(8, LE)?;
        let version = buf.pread_with::<u32>(16, LE)?;
        let size = buf.pread_with::<u32>(32, LE)? as usize;
        let compressed_size = buf.pread_with::<u32>(36, LE)? as usize;
        match version {
            1 => {
                let mut result =
                    zlib_decompress(&buf[0x38..])?[..size].to_vec();
                result.extend(zlib_decompress(&buf[0x38 + compressed_size..])?);
                let image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
                    ImageBuffer::from_vec(width, height, result)
                        .context("Invalid image resolution")?;
                Ok(Self {
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
