use super::{ResourceScheme, ResourceType};
use crate::{
    error::AkaibuError,
    util::{
        image::{resolve_color_table, resolve_color_table_without_alpha},
        zlib_decompress,
    },
};
use anyhow::Context;
use image::{buffer::ConvertBuffer, ImageBuffer};
use scroll::Pread;
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, Clone)]
pub(crate) enum CrxgScheme {
    Universal,
}

#[derive(Debug, Pread)]
struct CrxgHeader {
    magic: [u8; 4],
    unk0: u16,
    unk1: u16,
    width: u16,
    height: u16,
    unk2: u16,
    unk3: u16,
    has_alpha: u16,
    unk5: u16,
}

impl ResourceScheme for CrxgScheme {
    fn convert(&self, file_path: &Path) -> anyhow::Result<ResourceType> {
        let mut buf = Vec::with_capacity(1 << 20);
        let mut file = File::open(file_path)?;
        file.read_to_end(&mut buf)?;
        self.from_bytes(buf, file_path)
    }

    fn convert_from_bytes(
        &self,
        file_path: &std::path::Path,
        buf: Vec<u8>,
    ) -> anyhow::Result<super::ResourceType> {
        self.from_bytes(buf, file_path)
    }

    fn get_name(&self) -> String {
        format!(
            "[CRXG] {}",
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

impl CrxgScheme {
    fn from_bytes(
        &self,
        buf: Vec<u8>,
        _file_path: &Path,
    ) -> anyhow::Result<ResourceType> {
        let off = &mut 0;
        let header = buf.gread::<CrxgHeader>(off)?;
        let color_table = if header.has_alpha == 0x102 {
            let color_table = &buf[*off..*off + 0x400];
            *off += 0x400;
            color_table
        } else if header.has_alpha == 0x101 {
            let color_table = &buf[*off..*off + 0x300];
            *off += 0x300;
            color_table
        } else {
            &buf[..]
        };
        if header.unk2 > 2 {
            let headers_count = buf.gread::<u32>(off)? as usize;
            *off += headers_count * 16;
            if header.unk3 & 0x10 != 0 {
                *off += 4
            }
        }
        let image_data = zlib_decompress(&buf[*off..])?;
        match header.has_alpha {
            0 => self.bgr(&image_data, &header),
            1 => self.abgr(&image_data, &header),
            0x101 => self.color_table(&image_data, &header, color_table),
            0x102 => {
                self.color_table_with_alpha(&image_data, &header, color_table)
            }
            _ => {
                return Err(AkaibuError::Custom(format!(
                    "Invalid has_alpha value: {}",
                    header.has_alpha
                ))
                .into())
            }
        }
    }
    fn color_table_with_alpha(
        &self,
        image_data: &[u8],
        header: &CrxgHeader,
        color_table: &[u8],
    ) -> anyhow::Result<ResourceType> {
        let data = resolve_color_table(image_data, color_table);
        let image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
            ImageBuffer::from_vec(
                header.width as u32,
                header.height as u32,
                data,
            )
            .context("Invalid image resolution")?;
        Ok(ResourceType::RgbaImage {
            image: image.convert(),
        })
    }
    fn color_table(
        &self,
        image_data: &[u8],
        header: &CrxgHeader,
        color_table: &[u8],
    ) -> anyhow::Result<ResourceType> {
        let data = resolve_color_table_without_alpha(image_data, color_table);
        let image: ImageBuffer<image::Bgr<u8>, Vec<u8>> =
            ImageBuffer::from_vec(
                header.width as u32,
                header.height as u32,
                data,
            )
            .context("Invalid image resolution")?;
        Ok(ResourceType::RgbaImage {
            image: image.convert(),
        })
    }
    fn bgr(
        &self,
        image_data: &[u8],
        header: &CrxgHeader,
    ) -> anyhow::Result<ResourceType> {
        let data = self.resolve_pixels(&image_data, &header, 3)?;
        let image: ImageBuffer<image::Bgr<u8>, Vec<u8>> =
            ImageBuffer::from_vec(
                header.width as u32,
                header.height as u32,
                data,
            )
            .context("Invalid image resolution")?;
        Ok(ResourceType::RgbaImage {
            image: image.convert(),
        })
    }
    fn abgr(
        &self,
        image_data: &[u8],
        header: &CrxgHeader,
    ) -> anyhow::Result<ResourceType> {
        let data = self.resolve_pixels(&image_data, &header, 4)?;
        let mut image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
            ImageBuffer::from_vec(
                header.width as u32,
                header.height as u32,
                data,
            )
            .context("Invalid image resolution")?;
        for pixel in image.pixels_mut() {
            let red = pixel[3];
            let green = pixel[2];
            let blue = pixel[1];
            let alpha = pixel[0];
            pixel[3] = !alpha;
            pixel[0] = blue;
            pixel[1] = green;
            pixel[2] = red;
        }
        Ok(ResourceType::RgbaImage {
            image: image.convert(),
        })
    }
    fn resolve_pixels(
        &self,
        image_data: &[u8],
        header: &CrxgHeader,
        bytes_per_pixel: usize,
    ) -> anyhow::Result<Vec<u8>> {
        let mut dest = vec![
            0;
            header.width as usize
                * header.height as usize
                * bytes_per_pixel
        ];
        let image_off = &mut 0;
        let dest_off = &mut 0;
        let width = header.width as usize;
        for _ in 0..header.height {
            let x = image_data.gread::<u8>(image_off)?;
            match x {
                0 => {
                    ver0(
                        &image_data[*image_off..],
                        &mut dest[*dest_off..],
                        width,
                        bytes_per_pixel,
                    )?;
                    *image_off += width * bytes_per_pixel;
                }
                1 => {
                    ver1(
                        &image_data[*image_off..],
                        &dest[*dest_off - width * bytes_per_pixel..].to_vec(),
                        &mut dest[*dest_off..],
                        width,
                        bytes_per_pixel,
                    )?;
                    *image_off += width * bytes_per_pixel;
                }
                2 => {
                    ver2(
                        &image_data[*image_off..],
                        &dest[*dest_off - width * bytes_per_pixel..].to_vec(),
                        &mut dest[*dest_off..],
                        width,
                        bytes_per_pixel,
                    )?;
                    *image_off += width * bytes_per_pixel;
                }
                3 => {
                    ver3(
                        &image_data[*image_off..],
                        &dest[*dest_off - width * bytes_per_pixel..].to_vec(),
                        &mut dest[*dest_off..],
                        width,
                        bytes_per_pixel,
                    )?;
                    *image_off += width * bytes_per_pixel;
                }
                4 => {
                    let off = ver4(
                        &image_data[*image_off..],
                        &mut dest[*dest_off..],
                        width,
                        bytes_per_pixel,
                    )?;
                    *image_off += off;
                }
                _ => {
                    return Err(AkaibuError::Custom(
                        "Invalid image data".to_string(),
                    )
                    .into())
                }
            }
            *dest_off += width * bytes_per_pixel;
        }
        Ok(dest)
    }
}

fn ver0(
    src: &[u8],
    dest: &mut [u8],
    width: usize,
    bytes_per_pixel: usize,
) -> anyhow::Result<()> {
    let mut src_offset = 0;
    let mut dest_offset = 0;

    dest[dest_offset..dest_offset + bytes_per_pixel]
        .copy_from_slice(&src[src_offset..src_offset + bytes_per_pixel]);
    src_offset += bytes_per_pixel;
    dest_offset += bytes_per_pixel;

    for _ in 0..width - 1 {
        if bytes_per_pixel == 4 {
            dest[dest_offset] = dest[dest_offset - 4]
                .wrapping_add(src.pread::<u8>(src_offset)?);
            src_offset += 1;
            dest_offset += 1;

            dest[dest_offset] = dest[dest_offset - 4]
                .wrapping_add(src.pread::<u8>(src_offset)?);
            src_offset += 1;

            dest[dest_offset + 1] = dest[dest_offset - 3]
                .wrapping_add(src.pread::<u8>(src_offset)?);
            dest_offset += 1;
            src_offset += 1;

            dest[dest_offset + 1] = dest[dest_offset - 3]
                .wrapping_add(src.pread::<u8>(src_offset)?);
            src_offset += 1;
            dest_offset += 2;
        } else {
            dest[dest_offset] = dest[dest_offset - 3]
                .wrapping_add(src.pread::<u8>(src_offset)?);
            src_offset += 1;
            dest_offset += 1;

            dest[dest_offset] = dest[dest_offset - 3]
                .wrapping_add(src.pread::<u8>(src_offset)?);
            src_offset += 1;

            dest[dest_offset + 1] = dest[dest_offset - 2]
                .wrapping_add(src.pread::<u8>(src_offset)?);
            src_offset += 1;
            dest_offset += 2;
        }
    }
    Ok(())
}

fn ver1(
    src: &[u8],
    prev_line: &[u8],
    dest: &mut [u8],
    width: usize,
    bytes_per_pixel: usize,
) -> anyhow::Result<()> {
    let prev_line_offset = &mut 0;

    dest[..width * bytes_per_pixel]
        .copy_from_slice(&src[..width * bytes_per_pixel]);

    dest.iter_mut()
        .take(width * bytes_per_pixel)
        .try_for_each::<_, anyhow::Result<()>>(|b| {
            *b = b.wrapping_add(prev_line.gread::<u8>(prev_line_offset)?);
            Ok(())
        })
}

fn ver2(
    src: &[u8],
    prev_line: &[u8],
    dest: &mut [u8],
    width: usize,
    bytes_per_pixel: usize,
) -> anyhow::Result<()> {
    let prev_line_offset = &mut 0;

    dest[..width * bytes_per_pixel]
        .copy_from_slice(&src[..width * bytes_per_pixel]);

    dest.iter_mut()
        .skip(bytes_per_pixel)
        .take((width - 1) * bytes_per_pixel)
        .try_for_each::<_, anyhow::Result<()>>(|b| {
            *b = b.wrapping_add(prev_line.gread::<u8>(prev_line_offset)?);
            Ok(())
        })
}

fn ver3(
    src: &[u8],
    prev_line: &[u8],
    dest: &mut [u8],
    width: usize,
    bytes_per_pixel: usize,
) -> anyhow::Result<()> {
    let prev_line_offset = &mut bytes_per_pixel.clone();

    dest[..width * bytes_per_pixel]
        .copy_from_slice(&src[..width * bytes_per_pixel]);

    dest.iter_mut()
        .take((width - 1) * bytes_per_pixel)
        .try_for_each::<_, anyhow::Result<()>>(|b| {
            *b = b.wrapping_add(prev_line.gread::<u8>(prev_line_offset)?);
            Ok(())
        })
}

fn ver4(
    src: &[u8],
    dest: &mut [u8],
    width: usize,
    bytes_per_pixel: usize,
) -> anyhow::Result<usize> {
    let mut src_offset = 0;
    let mut dest_offset = 0;

    for _ in 0..bytes_per_pixel {
        let mut s = width;
        let mut c;
        while s > 0 {
            let b = src.pread::<u8>(src_offset)?;
            dest[dest_offset] = b;
            src_offset += 1;
            dest_offset += bytes_per_pixel;
            s -= 1;
            if s == 0 {
                break;
            }
            if b == src.pread::<u8>(src_offset)? {
                c = src.pread::<u8>(src_offset + 1)? as usize;
                src_offset += 2;
                s = s.wrapping_sub(c);
                while c > 0 {
                    dest[dest_offset] = b;
                    c -= 1;
                    dest_offset += bytes_per_pixel;
                }
            }
        }
        dest_offset = dest_offset
            .wrapping_sub(width * bytes_per_pixel)
            .wrapping_add(1);
    }

    Ok(src_offset)
}
