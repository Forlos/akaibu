use crate::{
    archive::{self, FileEntry},
    error::AkaibuError,
    util::simd::{packuswb0, paddw, psrlw, psubb, punpcklbw0},
};

use super::{ResourceScheme, ResourceType};
use anyhow::Context;
use image::{buffer::ConvertBuffer, ImageBuffer};
use scroll::{Pread, LE};
use std::{convert::TryInto, fs::File, io::Read, path::Path};

#[derive(Debug, Clone)]
pub(crate) enum PgdScheme {
    Universal,
}

#[derive(Debug, Pread)]
struct GeHeader {
    magic: [u8; 2],
    pixel_data_offset: u16,
    unk0: u32,
    unk1: u32,
    width: u32,
    height: u32,
    width2: u32,
    height2: u32,
    version: u16,
}

#[derive(Debug, Pread)]
struct Pgd3Header {
    magic: [u8; 4],
    left_offset: u16,
    top_offset: u16,
    width: u16,
    height: u16,
    bpp: u16,
    parent_file_name: [u8; 34],
}

impl ResourceScheme for PgdScheme {
    fn convert(
        &self,
        file_path: &std::path::Path,
    ) -> anyhow::Result<super::ResourceType> {
        let mut buf = Vec::with_capacity(1 << 20);
        let mut file = File::open(file_path)?;
        file.read_to_end(&mut buf)?;
        self.from_bytes(buf, file_path, None)
    }

    fn convert_from_bytes(
        &self,
        file_path: &std::path::Path,
        buf: Vec<u8>,
        archive: Option<&Box<dyn archive::Archive>>,
    ) -> anyhow::Result<super::ResourceType> {
        self.from_bytes(buf, file_path, archive)
    }

    fn get_name(&self) -> String {
        format!(
            "[Pgd] {}",
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

impl PgdScheme {
    fn from_bytes(
        &self,
        buf: Vec<u8>,
        file_path: &Path,
        archive: Option<&Box<dyn archive::Archive>>,
    ) -> anyhow::Result<ResourceType> {
        match &buf[..4] {
            [0x47, 0x45, ..] => {
                let (pixels, width, height) = ge_image(buf)?;
                let image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
                    ImageBuffer::from_vec(width, height, pixels)
                        .context("Invalid image resolution")?;
                Ok(ResourceType::RgbaImage {
                    image: image.convert(),
                })
            }
            [0x50, 0x47, 0x44, 0x32] => {
                return Err(AkaibuError::Custom(
                    "Unsupported image format PGD2".to_string(),
                )
                .into())
            }
            [0x50, 0x47, 0x44, 0x33] => {
                pgd3_image(buf, archive, file_path)
                /* return Err(AkaibuError::Custom(
                    "Unsupported image format PGD3".to_string(),
                )
                .into()) */
            }
            _ => {
                return Err(AkaibuError::Custom(format!(
                    "Invalid magic value for Pgd {:?}",
                    &buf[..4]
                ))
                .into())
            }
        }
    }
}

fn ge_image(buf: Vec<u8>) -> anyhow::Result<(Vec<u8>, u32, u32)> {
    let off = &mut 0;
    let header = buf.gread::<GeHeader>(off)?;
    if header.version != 3 {
        return Err(AkaibuError::Custom(format!(
            "Unsupported version for GE image {}",
            header.version
        ))
        .into());
    }

    let pixel_data = &decompress(&buf[header.pixel_data_offset as usize..])?;
    let bytes_per_pixel = pixel_data.pread_with::<u16>(2, LE)? as usize >> 3;

    let pixel_data = parse_pixels(
        &pixel_data[8..],
        header.width as usize,
        header.height as usize,
        bytes_per_pixel,
    )?;
    Ok((pixel_data, header.width, header.height))
}

// TODO: Add possibility for getting parent image from archive/file system so formats like this,
// expecting child image layering on top of parent image work.
fn pgd3_image(
    buf: Vec<u8>,
    archive: Option<&Box<dyn archive::Archive>>,
    file_path: &Path,
) -> anyhow::Result<ResourceType> {
    let off = &mut 0;
    let header = buf.gread::<Pgd3Header>(off)?;

    let parent_name = String::from_utf8(
        header
            .parent_file_name
            .iter()
            .take_while(|b| **b != 0)
            .map(|b| *b)
            .collect::<Vec<u8>>(),
    )?
    .to_uppercase();

    let parent = match archive {
        Some(archive) => ge_image(
            archive
                .extract(&FileEntry {
                    file_name: parent_name.clone(),
                    full_path: parent_name.clone().into(),
                    file_offset: 0,
                    file_size: 0,
                })?
                .contents
                .to_vec(),
        )?,
        None => {
            let mut path = file_path
                .parent()
                .context("Invalid path: At root dir")?
                .to_path_buf();
            path.push(&parent_name);
            match File::open(path) {
                Ok(mut file) => {
                    let mut buf = Vec::with_capacity(1 << 20);
                    file.read_to_end(&mut buf)?;
                    ge_image(buf)?
                }
                Err(_) => {
                    return Err(AkaibuError::Custom(format!(
                        "Could not find parent file: {}",
                        parent_name
                    ))
                    .into())
                }
            }
        }
    };

    let mut parent_image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
        ImageBuffer::from_vec(parent.1, parent.2, parent.0)
            .context("Invalid image resolution")?;

    let pixel_data = parse_pixels(
        &decompress(&buf[*off..])?,
        header.width as usize,
        header.height as usize,
        header.bpp as usize >> 3,
    )?;

    let image: ImageBuffer<image::Bgra<u8>, Vec<u8>> = ImageBuffer::from_vec(
        header.width as u32,
        header.height as u32,
        pixel_data,
    )
    .context("Invalid image resolution")?;

    for x in header.left_offset as u32
        ..header.left_offset as u32 + header.width as u32
    {
        for y in header.top_offset as u32
            ..header.top_offset as u32 + header.height as u32
        {
            let a = image.get_pixel(
                x - header.left_offset as u32,
                y - header.top_offset as u32,
            );
            let b = parent_image.get_pixel_mut(x, y);
            for i in 0..header.bpp as usize >> 3 {
                b[i] ^= a[i];
            }
        }
    }

    Ok(ResourceType::RgbaImage {
        image: parent_image.convert(),
    })
}

fn decompress(src: &[u8]) -> anyhow::Result<Vec<u8>> {
    let dest_size = src.pread_with::<u32>(0, LE)? as usize;
    let cur_src = &src[8..];

    let src_index = &mut 0;
    let dest_index = &mut 0;
    let mut dest = vec![0; dest_size];

    let mut d = cur_src.gread::<u8>(src_index)?;
    let mut dh = 0;
    while *dest_index < dest_size {
        if (d & 1) != 0 {
            let mut s = *dest_index;
            let mut a = cur_src.gread_with::<u16>(src_index, LE)?;
            if (a & 8) == 0 {
                let mut a = (a as u32) << 8;
                a |= cur_src.gread::<u8>(src_index)? as u32;
                let mut c = a;
                a >>= 12;
                c &= 0xFFF;
                s -= a as usize;
                c += 4;
                dest.copy_within(s..s + c as usize, *dest_index);
                *dest_index += c as usize;
            } else {
                let mut c = a as u8;
                a >>= 4;
                c &= 7;
                c += 4;
                s -= a as usize;
                dest.copy_within(s..s + c as usize, *dest_index);
                *dest_index += c as usize;
            }
        } else {
            let c = cur_src.gread::<u8>(src_index)? as usize;
            dest[*dest_index..*dest_index + c]
                .copy_from_slice(&cur_src[*src_index..*src_index + c]);
            *dest_index += c;
            *src_index += c;
        }
        d >>= 1;
        dh += 1;
        dh &= 7;
        if dh == 0 && *src_index < cur_src.len() {
            d = cur_src.gread::<u8>(src_index)?;
        }
    }

    Ok(dest)
}

fn parse_pixels(
    src: &[u8],
    width: usize,
    height: usize,
    bytes_per_pixel: usize,
) -> anyhow::Result<Vec<u8>> {
    let src_index = &mut (0 + height);
    let dest_index = &mut 0;
    let mut dest = vec![0; width * height * 4];
    let line_array = &src[..height];

    let mut cur = [0xFF; 4];
    let mut prev = [0xFF; 4];
    for a in line_array {
        if (a & 1) == 0 {
            if (a & 2) == 0 {
                if (a & 4) != 0 {
                    let mut prev_line_index = *dest_index - width * 4;
                    prev[0..bytes_per_pixel].copy_from_slice(
                        &src[*src_index..*src_index + bytes_per_pixel],
                    );
                    dest[*dest_index..*dest_index + 4].copy_from_slice(&prev);

                    *src_index += bytes_per_pixel;
                    *dest_index += 4;
                    prev_line_index += 4;

                    let mut mm3 = [0xFFu8; 4];

                    for _ in 0..width - 1 {
                        let mm1 = dest[prev_line_index..prev_line_index + 4]
                            .try_into()?;
                        let mm2 = dest[*dest_index - 4..*dest_index - 4 + 4]
                            .try_into()?;
                        mm3[..bytes_per_pixel].copy_from_slice(
                            &src[*src_index..*src_index + bytes_per_pixel],
                        );
                        let mut mm1 = punpcklbw0(mm1);
                        let mm2 = punpcklbw0(mm2);
                        paddw(&mut mm1, &mm2)?;
                        psrlw(&mut mm1, 1)?;
                        let mut mm1 = packuswb0(mm1)?;
                        psubb(&mut mm1, &mm3, bytes_per_pixel);

                        dest[*dest_index..*dest_index + 4]
                            .copy_from_slice(&mm1);

                        *src_index += bytes_per_pixel;
                        *dest_index += 4;
                        prev_line_index += 4;
                    }
                }
            } else {
                let mut prev_line_index = *dest_index - width * 4;
                for _ in 0..width {
                    cur[0..bytes_per_pixel].copy_from_slice(
                        &src[*src_index..*src_index + bytes_per_pixel],
                    );
                    prev[0..bytes_per_pixel].copy_from_slice(
                        &dest[prev_line_index
                            ..prev_line_index + bytes_per_pixel],
                    );
                    psubb(&mut prev, &cur, bytes_per_pixel);
                    dest[*dest_index..*dest_index + 4].copy_from_slice(&prev);

                    *src_index += bytes_per_pixel;
                    *dest_index += 4;
                    prev_line_index += 4;
                }
            }
        } else {
            prev[0..bytes_per_pixel].copy_from_slice(
                &src[*src_index..*src_index + bytes_per_pixel],
            );
            dest[*dest_index..*dest_index + 4].copy_from_slice(&prev);
            *src_index += bytes_per_pixel;
            *dest_index += 4;
            for _ in 0..width - 1 {
                cur[0..bytes_per_pixel].copy_from_slice(
                    &src[*src_index..*src_index + bytes_per_pixel],
                );
                psubb(&mut prev, &cur, bytes_per_pixel);
                dest[*dest_index..*dest_index + 4].copy_from_slice(&prev);

                *src_index += bytes_per_pixel;
                *dest_index += 4;
            }
        }
    }
    Ok(dest)
}
