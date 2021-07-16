use super::{ResourceScheme, ResourceType};
use anyhow::Context;
use image::{buffer::ConvertBuffer, ImageBuffer};
use scroll::{Pread, LE};
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, Clone)]
pub(crate) enum PgdScheme {
    Universal,
}

#[derive(Debug, Pread)]
struct PgdHeader {
    magic: [u8; 2],
    pixel_data_offset: u16,
    unk0: u32,
    unk1: u32,
    width: u32,
    height: u32,
    width2: u32,
    height2: u32,
    bytes_per_pixel: u16,
}

impl ResourceScheme for PgdScheme {
    fn convert(
        &self,
        file_path: &std::path::Path,
    ) -> anyhow::Result<super::ResourceType> {
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
        _file_path: &Path,
    ) -> anyhow::Result<ResourceType> {
        let off = &mut 0;
        let header = buf.gread::<PgdHeader>(off)?;

        let pixel_data = parse_pixels(
            &decompress(&buf[header.pixel_data_offset as usize..])?,
            header.width as usize,
            header.height as usize,
        )?;

        let image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
            ImageBuffer::from_vec(
                header.width as u32,
                header.height as u32,
                pixel_data,
            )
            .context("Invalid image resolution")?;
        Ok(ResourceType::RgbaImage {
            image: image.convert(),
        })
    }
}

fn decompress(mut src: &[u8]) -> anyhow::Result<Vec<u8>> {
    let dest_size = src.pread_with::<u32>(0, LE)? as usize;
    src = &src[8..];

    let src_index = &mut 0;
    let dest_index = &mut 0;
    let mut dest = vec![0; dest_size];

    let mut d = src.gread::<u8>(src_index)?;
    let mut dh = 0;
    while *dest_index < dest_size {
        if (d & 1) != 0 {
            let mut s = *dest_index;
            let mut a = src.gread_with::<u16>(src_index, LE)?;
            if (a & 8) == 0 {
                let mut a = (a as u32) << 8;
                a |= src.gread::<u8>(src_index)? as u32;
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
            let c = src.gread::<u8>(src_index)? as usize;
            dest[*dest_index..*dest_index + c]
                .copy_from_slice(&src[*src_index..*src_index + c]);
            *dest_index += c;
            *src_index += c;
        }
        d >>= 1;
        dh += 1;
        dh &= 7;
        if dh == 0 {
            d = src.gread::<u8>(src_index)?;
        }
    }

    Ok(dest)
}

fn parse_pixels(
    src: &[u8],
    width: usize,
    height: usize,
) -> anyhow::Result<Vec<u8>> {
    let src_index = &mut (8 + height);
    let dest_index = &mut 0;
    let mut dest = vec![0; width * height * 4];
    let line_array = &src[8..height + 8];

    let mut cur = [0xFF; 4];
    let mut prev = [0xFF; 4];
    for a in line_array {
        prev[3] = 0xFF;
        if (a & 1) == 0 {
            if (a & 2) == 0 {
                if (a & 4) != 0 {
                    let mut prev_line_index = *dest_index - width * 4;
                    prev[0..3]
                        .copy_from_slice(&src[*src_index..*src_index + 3]);
                    dest[*dest_index..*dest_index + 4].copy_from_slice(&prev);

                    let mut c = *dest_index + 1;

                    *src_index += 3;
                    *dest_index += 4;
                    prev_line_index += 4;

                    if width > 1 {
                        for _ in 0..width - 1 {
                            let a = dest[c + 1] as u32;
                            cur[0..3].copy_from_slice(
                                &src[*src_index..*src_index + 3],
                            );
                            prev[0..3].copy_from_slice(
                                &dest[prev_line_index..prev_line_index + 3],
                            );
                            prev[2] = (prev[2] as u32)
                                .wrapping_add(a)
                                .wrapping_shr(1)
                                .wrapping_sub(cur[2] as u32)
                                as u8;
                            let cx =
                                dest[prev_line_index + width * 4 - 4] as u32;
                            prev[0] = (prev[0] as u32)
                                .wrapping_add(cx)
                                .wrapping_shr(1)
                                .wrapping_sub(cur[0] as u32)
                                as u8;
                            let cx = dest[c] as u32;
                            prev[1] = (prev[1] as u32)
                                .wrapping_add(cx)
                                .wrapping_shr(1)
                                .wrapping_sub(cur[1] as u32)
                                as u8;

                            dest[*dest_index..*dest_index + 4]
                                .copy_from_slice(&prev);

                            c += 4;
                            *src_index += 3;
                            *dest_index += 4;
                            prev_line_index += 4;
                        }
                    }
                }
            } else {
                if width != 0 {
                    let mut prev_line_index = *dest_index - width * 4;
                    for _ in 0..width {
                        cur[0..3]
                            .copy_from_slice(&src[*src_index..*src_index + 3]);
                        prev[0..3].copy_from_slice(
                            &dest[prev_line_index..prev_line_index + 3],
                        );
                        psub3(&mut prev, &cur);
                        dest[*dest_index..*dest_index + 4]
                            .copy_from_slice(&prev);

                        *src_index += 3;
                        *dest_index += 4;
                        prev_line_index += 4;
                    }
                }
            }
        } else {
            prev[0..3].copy_from_slice(&src[*src_index..*src_index + 3]);
            dest[*dest_index..*dest_index + 4].copy_from_slice(&prev);
            *src_index += 3;
            *dest_index += 4;
            if width > 1 {
                for _ in 0..width - 1 {
                    cur[0..3].copy_from_slice(&src[*src_index..*src_index + 3]);
                    psub3(&mut prev, &cur);
                    dest[*dest_index..*dest_index + 4].copy_from_slice(&prev);

                    *src_index += 3;
                    *dest_index += 4;
                }
            }
        }
    }
    Ok(dest)
}

fn psub3(p1: &mut [u8; 4], p2: &[u8; 4]) {
    for i in 0..3 {
        p1[i] = p1[i].wrapping_sub(p2[i]);
    }
}
