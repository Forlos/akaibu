use super::{ResourceScheme, ResourceType};
use crate::{error::AkaibuError, util::image::resolve_color_table};
use anyhow::Context;
use image::{buffer::ConvertBuffer, ImageBuffer};
use scroll::{Pread, LE};
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, Clone)]
pub(crate) enum G00Scheme {
    Universal,
}

#[derive(Debug, Pread)]
struct G00Header {
    version: u8,
    width: u16,
    height: u16,
}

impl ResourceScheme for G00Scheme {
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
            "[G00] {}",
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

impl G00Scheme {
    fn from_bytes(&self, buf: Vec<u8>) -> anyhow::Result<ResourceType> {
        let header = buf.pread::<G00Header>(0)?;
        match header.version {
            0 => Self::version0(&buf[5..], header),
            1 => Self::version1(&buf[5..], header),
            2 => Self::version2(&buf[5..]),
            _ => Err(AkaibuError::Custom(format!(
                "Not supported version {}",
                header.version
            ))
            .into()),
        }
    }
    fn version0(buf: &[u8], header: G00Header) -> anyhow::Result<ResourceType> {
        let uncompressed_size = buf.pread_with::<u32>(4, LE)?;
        let pixels = Self::decompress0(&buf[8..], uncompressed_size as usize)?;
        let image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
            ImageBuffer::from_vec(
                header.width as u32,
                header.height as u32,
                pixels,
            )
            .context("Invalid image resolution")?;
        Ok(ResourceType::RgbaImage {
            image: image.convert(),
        })
    }
    fn version1(buf: &[u8], header: G00Header) -> anyhow::Result<ResourceType> {
        let uncompressed_size = buf.pread_with::<u32>(4, LE)?;
        let data = Self::decompress2(&buf[8..], uncompressed_size as usize)?;
        let color_table_size = data.pread_with::<u16>(0, LE)? as usize;
        let color_table = &data[2..color_table_size * 4 + 2];
        let color_index_table = &data[color_table_size * 4 + 2..];
        let pixels = resolve_color_table(color_index_table, color_table);
        let image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
            ImageBuffer::from_vec(
                header.width as u32,
                header.height as u32,
                pixels,
            )
            .context("Invalid image resolution")?;
        Ok(ResourceType::RgbaImage {
            image: image.convert(),
        })
    }
    fn version2(buf: &[u8]) -> anyhow::Result<ResourceType> {
        let mut off = 0;
        let subimage_count = buf.gread_with::<u32>(&mut off, LE)? as usize;
        let mut subimages = Vec::with_capacity(subimage_count);
        for _ in 0..subimage_count {
            let subimage = buf.gread::<SubImage>(&mut off)?;
            if subimage.right != 0 && subimage.bottom != 0 {
                subimages.push(subimage);
            }
        }
        let _compressed_size = buf.gread::<u32>(&mut off)?;
        let uncompressed_size = buf.gread::<u32>(&mut off)?;
        let data = Self::decompress2(&buf[off..], uncompressed_size as usize)?;
        let mut sprite_offsets = Vec::with_capacity(subimage_count);
        let mut data_off = 4;
        for _ in 0..subimage_count {
            let offset = data.gread_with::<u32>(&mut data_off, LE)? as usize;
            let size = data.gread_with::<u32>(&mut data_off, LE)?;
            if size != 0 {
                sprite_offsets.push(offset);
            }
        }
        let mut sprites = Vec::with_capacity(sprite_offsets.len());
        for offset in sprite_offsets {
            let sprite = data.pread::<Sprite>(offset)?;
            let mut chunks = Vec::with_capacity(sprite.chunk_count as usize);
            let mut chunk_offset = offset + 0x74;
            for _ in 0..sprite.chunk_count {
                let chunk = data.pread::<Chunk>(chunk_offset)?;
                let chunk_size =
                    chunk.width as usize * chunk.height as usize * 4;
                chunks.push((chunk, chunk_offset + 0x5C));
                chunk_offset += chunk_size + 0x5C;
            }
            sprites.push((sprite, chunks));
        }
        let mut images = Vec::with_capacity(sprites.len());
        for ((_sprite, chunks), subimage) in sprites.iter().zip(subimages) {
            let width = subimage.right - subimage.left + 1;
            let height = subimage.bottom - subimage.top + 1;
            let mut pixels = vec![0; width as usize * height as usize * 4];
            for (chunk, offset) in chunks {
                let padding = (width as usize - chunk.width as usize) * 4;
                let pixels_index = (chunk.top as usize * width as usize * 4)
                    + chunk.left as usize * 4;
                Self::read_chunk(
                    &data[*offset..],
                    &mut pixels,
                    pixels_index,
                    padding,
                    chunk.height,
                    chunk.width,
                )
            }
            let image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
                ImageBuffer::from_vec(width as u32, height as u32, pixels)
                    .context("Invalid image resolution")?;
            images.push(image.convert());
        }
        Ok(ResourceType::SpriteSheet { sprites: images })
    }
    fn decompress0(src: &[u8], dest_len: usize) -> anyhow::Result<Vec<u8>> {
        let mut dest = Vec::with_capacity(dest_len);
        let mut start = true;
        let src_index = &mut 0;
        let mut dl = 0;
        let mut d = 0;
        while dest.len() != dest_len {
            if start {
                dl = src.gread::<u8>(src_index)?;
                d = 8;
                start = false;
            }
            if dest.len() == dest_len {
                return Ok(dest);
            }
            if (dl & 1) == 0 {
                let mut a = src.gread_with::<u16>(src_index, LE)?;
                let counter = (a & 0x0F) + 1;
                a = (a >> 4) << 2;
                let mut temp_index = dest.len() - a as usize;
                for _ in 0..counter {
                    let temp = dest[temp_index..temp_index + 4].to_vec();
                    dest.extend_from_slice(&temp);
                    temp_index += 4;
                }
            } else {
                dest.extend_from_slice(&src[*src_index..*src_index + 3]);
                *src_index += 3;
                dest.push(0xFF);
            }
            dl >>= 1;
            d -= 1;
            if d == 0 {
                start = true;
            }
        }
        Ok(dest)
    }
    fn decompress2(src: &[u8], dest_len: usize) -> anyhow::Result<Vec<u8>> {
        let mut dest = Vec::with_capacity(dest_len);
        let mut start = true;
        let src_index = &mut 0;
        let mut dl = 0;
        let mut d = 0;
        while dest.len() != dest_len {
            if start {
                dl = src.gread::<u8>(src_index)?;
                d = 8;
                start = false;
            }
            if dest.len() == dest_len {
                return Ok(dest);
            }
            if (dl & 1) == 0 {
                let mut a = src.gread_with::<u16>(src_index, LE)?;
                let counter = (a & 0x0F) + 2;
                a >>= 4;
                let mut temp_index = dest.len() - a as usize;
                for _ in 0..counter {
                    dest.push(dest.gread::<u8>(&mut temp_index)?);
                    if dest.len() == dest_len {
                        return Ok(dest);
                    }
                }
            } else {
                dest.push(src.gread::<u8>(src_index)?);
            }
            dl >>= 1;
            d -= 1;
            if d == 0 {
                start = true;
            }
        }
        Ok(dest)
    }
    fn read_chunk(
        src: &[u8],
        dest: &mut [u8],
        mut dest_index: usize,
        padding: usize,
        height: u16,
        width: u16,
    ) {
        let mut src_index = 0;
        for _ in 0..height {
            for _ in 0..width {
                dest[dest_index..dest_index + 4]
                    .copy_from_slice(&src[src_index..src_index + 4]);
                dest_index += 4;
                src_index += 4;
            }
            dest_index += padding;
        }
    }
}

#[derive(Debug, Pread)]
struct SubImage {
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    unk4: u32,
    unk5: u32,
}

#[derive(Debug, Pread)]
struct Sprite {
    unk0: u16,
    chunk_count: u16,
    a: u32,
    b: u32,
    width: u32,
    height: u32,
    unk1: u32,
    unk2: u32,
    full_width: u32,
    full_height: u32,
}

#[derive(Debug, Pread)]
struct Chunk {
    left: u16,
    top: u16,
    flag: u16,
    width: u16,
    height: u16,
}
