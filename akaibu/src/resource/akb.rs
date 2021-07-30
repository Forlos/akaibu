use super::{ResourceScheme, ResourceType};
use crate::{archive, error::AkaibuError, util::image::bitmap_to_png};
use anyhow::Context;
use image::{buffer::ConvertBuffer, ImageBuffer, Pixel};
use scroll::Pread;
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, Clone)]
pub(crate) enum AkbScheme {
    Universal,
}

#[derive(Debug, Pread)]
struct AkbHeader {
    magic: [u8; 4],
    width: u16,
    height: u16,
    compression: u32,
    fill: u32,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

impl ResourceScheme for AkbScheme {
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
        _archive: Option<&Box<dyn archive::Archive>>,
    ) -> anyhow::Result<ResourceType> {
        self.from_bytes(buf)
    }

    fn get_name(&self) -> String {
        format!(
            "[AKB] {}",
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

impl AkbScheme {
    fn from_bytes(&self, buf: Vec<u8>) -> anyhow::Result<ResourceType> {
        let header = buf.pread::<AkbHeader>(0)?;
        let data_offset = match &header.magic {
            b"AKB " => 32,
            b"AKB+" => 64,
            _ => {
                return Err(AkaibuError::Custom(format!(
                    "Invalid AKB magic {:X?}",
                    header.magic
                ))
                .into())
            }
        };
        let pixels = Self::transform(
            bitmap_to_png(
                Self::decompress(&buf[data_offset..], &header),
                header.width as usize * 4,
            ),
            &header,
            header.left as usize * 4
                + header.top as usize * 4 * header.width as usize,
        );
        let mut image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
            ImageBuffer::from_vec(
                header.width as u32,
                header.height as u32,
                pixels,
            )
            .context("Invalid image resolution")?;
        Self::apply_filters(&mut image, &header);
        Ok(ResourceType::RgbaImage {
            image: image.convert(),
        })
    }
    fn decompress(buf: &[u8], akb: &AkbHeader) -> Vec<u8> {
        let dest_len = akb.width as usize * akb.height as usize * 4;
        let w_in = (akb.right as usize - akb.left as usize) * 4;
        let w_out =
            (akb.width as usize - (akb.right as usize - akb.left as usize)) * 4;
        let write_index = akb.left as usize * 4
            + (akb.height as usize - akb.bottom as usize)
                * 4
                * akb.width as usize;
        if akb.compression & 0x40_00_00_00 == 0 {
            Self::decompress3(buf, dest_len, w_in, w_out, write_index)
        } else {
            Self::decompress2(buf, dest_len, w_in, w_out, write_index)
        }
    }
    fn decompress2(
        buf: &[u8],
        dest_len: usize,
        w_in: usize,
        w_out: usize,
        write_index: usize,
    ) -> Vec<u8> {
        let mut lookup_table = vec![0u8; 4096];
        let mut dest = vec![0u8; dest_len];
        let mut x = 0_u16;
        let mut lookup_index = 4078;
        let mut bytes_read = 0;
        let mut bytes_written = write_index;
        let mut cur_index = w_in;
        while bytes_read < buf.len() {
            x >>= 1;
            if (x & 0x100) == 0 {
                x = buf[bytes_read] as u16;
                bytes_read += 1;
                x |= 0xFF00;
            }
            let bl = buf[bytes_read];
            bytes_read += 1;
            if ((x & 0xFF) & 1) == 0 {
                let cl = buf[bytes_read];
                bytes_read += 1;
                let mut s = cl as u16;
                let mut d = s as u16;
                let mut c = bl as u16;
                d &= 0xF0;
                s &= 0x0F;
                d <<= 4;
                s += 3;
                d |= c;
                c = s;
                if c > 0 {
                    s = d;
                    let mut counter = c;
                    while counter != 0 {
                        c = s & 0xFFF;
                        d = lookup_table[c as usize] as u16;
                        dest[bytes_written] = d as u8;
                        bytes_written += 1;
                        cur_index -= 1;
                        c = cur_index as u16 & 3;
                        if c == 1 {
                            bytes_written += 1;
                            cur_index -= 1;
                            if cur_index == 0 {
                                bytes_written += w_out;
                                cur_index = w_in;
                            }
                        }
                        c = lookup_index;
                        lookup_index += 1;
                        lookup_index &= 0xFFF;
                        lookup_table[c as usize] = d as u8;

                        s += 1;
                        counter -= 1;
                    }
                }
            } else {
                dest[bytes_written] = bl;
                bytes_written += 1;
                cur_index -= 1;
                let mut c = cur_index as u16 & 3;
                if c == 1 {
                    bytes_written += 1;
                    cur_index -= 1;
                    if cur_index == 0 {
                        bytes_written += w_out;
                        cur_index = w_in;
                    }
                }

                c = lookup_index;
                lookup_index += 1;
                lookup_index &= 0xFFF;
                lookup_table[c as usize] = bl
            }
        }
        dest
    }
    fn decompress3(
        buf: &[u8],
        dest_len: usize,
        w_in: usize,
        w_out: usize,
        write_index: usize,
    ) -> Vec<u8> {
        let mut lookup_table = vec![0u8; 4096];
        let mut dest = vec![0u8; dest_len];
        let mut x = 0_u16;
        let mut lookup_index = 4078;
        let mut bytes_read = 0;
        let mut bytes_written = write_index;
        let mut cur_index = w_in;
        while bytes_read < buf.len() {
            x >>= 1;
            if (x & 0x100) == 0 {
                x = buf[bytes_read] as u16;
                bytes_read += 1;
                x |= 0xFF00;
            }
            let mut bl = buf[bytes_read];
            bytes_read += 1;
            if ((x & 0xFF) & 1) == 0 {
                let cl = buf[bytes_read];
                bytes_read += 1;
                let mut s = cl as u16;
                let mut d = s as u16;
                let mut c = bl as u16;
                d &= 0xF0;
                s &= 0x0F;
                d <<= 4;
                s += 3;
                d |= c;
                c = s;
                if c > 0 {
                    let mut counter = c;
                    while counter != 0 {
                        c = d & 0xFFF;
                        bl = lookup_table[c as usize];
                        dest[bytes_written] = bl;
                        bytes_written += 1;
                        cur_index -= 1;
                        if cur_index == 0 {
                            bytes_written += w_out;
                            cur_index = w_in;
                        }
                        c = lookup_index;
                        lookup_index += 1;
                        lookup_index &= 0xFFF;
                        lookup_table[c as usize] = bl;

                        d += 1;
                        counter -= 1;
                    }
                }
            } else {
                dest[bytes_written] = bl;
                bytes_written += 1;
                cur_index -= 1;
                if cur_index == 0 {
                    bytes_written += w_out;
                    cur_index = w_in;
                }

                let c = lookup_index;
                lookup_index += 1;
                lookup_index &= 0xFFF;
                lookup_table[c as usize] = bl;
            }
        }
        dest
    }
    fn transform(buf: Vec<u8>, akb: &AkbHeader, start_index: usize) -> Vec<u8> {
        let w_in = (akb.right - akb.left) as usize;
        let h_in = (akb.bottom - akb.top) as usize;
        if w_in == 0 || h_in == 0 {
            return buf;
        }

        let mut dest =
            Vec::with_capacity(akb.width as usize * akb.height as usize * 4);
        dest.extend_from_slice(&buf[..start_index]);

        let line = &buf[start_index..start_index + w_in * 4];
        let mut prev = line[..4].to_vec();
        dest.extend_from_slice(&prev);
        for pixel in line[4..].chunks(4) {
            prev = (pixel[0].wrapping_add(prev[0]) as u32
                + ((pixel[1].wrapping_add(prev[1]) as u32) << 8)
                + ((pixel[2].wrapping_add(prev[2]) as u32) << 16)
                + ((pixel[3].wrapping_add(prev[3]) as u32) << 24))
                .to_le_bytes()
                .to_vec();
            dest.extend_from_slice(&prev);
        }
        dest.extend_from_slice(
            &buf[start_index + w_in * 4..start_index + akb.width as usize * 4],
        );

        for (line_index, line) in buf[start_index + akb.width as usize * 4
            ..start_index + (h_in - 1) * akb.width as usize * 4]
            .chunks(akb.width as usize * 4)
            .enumerate()
        {
            let cur_line = &line[..w_in * 4];
            let prev_line = dest[start_index
                + (line_index * akb.width as usize * 4)
                ..start_index + ((line_index + 1) * akb.width as usize * 4)]
                .to_vec();
            for (pixel, prev) in cur_line.chunks(4).zip(prev_line.chunks(4)) {
                let cur = pixel[0].wrapping_add(prev[0]) as u32
                    + ((pixel[1].wrapping_add(prev[1]) as u32) << 8)
                    + ((pixel[2].wrapping_add(prev[2]) as u32) << 16)
                    + ((pixel[3].wrapping_add(prev[3]) as u32) << 24);
                dest.extend_from_slice(&cur.to_le_bytes());
            }
            dest.extend_from_slice(&line[w_in * 4..]);
        }

        dest.extend_from_slice(&buf[dest.len()..]);
        dest
    }
    fn apply_filters(
        image: &mut ImageBuffer<image::Bgra<u8>, Vec<u8>>,
        akb: &AkbHeader,
    ) {
        if akb.compression & 0x40000000 != 0 {
            let new_alpha = akb.compression as u8;
            for pixel in image.pixels_mut() {
                pixel.apply_with_alpha(|c| c, |_| new_alpha);
            }
        }
        if akb.compression & 0x80000000 != 0 {
            let fill = akb.fill;
            let b = fill as u8;
            let g = (fill >> 8) as u8;
            let r = (fill >> 16) as u8;
            let a = (fill >> 24) as u8;
            let height_range = akb.top..akb.bottom - 1;
            let width_range = akb.left..akb.right;

            for (width, height, pixel) in image.enumerate_pixels_mut() {
                if !height_range.contains(&height)
                    || !width_range.contains(&width)
                {
                    pixel[0] = b;
                    pixel[1] = g;
                    pixel[2] = r;
                    pixel[3] = a;
                }
            }
        }
    }
}
