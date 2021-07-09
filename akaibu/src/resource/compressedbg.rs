use super::{ResourceScheme, ResourceType};
use crate::error::AkaibuError;
use anyhow::Context;
use image::{buffer::ConvertBuffer, ImageBuffer};
use scroll::{Pread, LE};
use std::{convert::TryInto, fs::File, io::Read, path::Path};

#[derive(Debug, Clone)]
pub(crate) enum BgScheme {
    Universal,
}

#[derive(Debug, Pread)]
struct BgHeader {
    magic: [u8; 16],
    width: u16,
    height: u16,
    bpp: u16,
    unk0: u16,
    unk1: u32,
    unk2: u32,
    unk3: u32,
    prng_seed: u32,
    decrypt_data_size: u32,
    unk4: u32,
}

impl ResourceScheme for BgScheme {
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
            "[CompressedBg] {}",
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

impl BgScheme {
    fn from_bytes(
        &self,
        buf: Vec<u8>,
        _file_path: &Path,
    ) -> anyhow::Result<ResourceType> {
        let off = &mut 0;
        let header = buf.gread::<BgHeader>(off)?;
        if header.decrypt_data_size < 256 {
            return Err(AkaibuError::Custom(
                "Unsupported CompressedBg image".to_string(),
            )
            .into());
        }
        if header.bpp != 24 && header.bpp != 32 {
            return Err(AkaibuError::Custom(format!(
                "Unsupported bpp value {}",
                header.bpp
            ))
            .into());
        }
        let mut decrypt_data =
            buf[*off..*off + header.decrypt_data_size as usize].to_vec();
        *off += header.decrypt_data_size as usize;
        let mut state = header.prng_seed;
        decrypt_data.iter_mut().for_each(|b| {
            let (val, new_state) = prng(state);
            state = new_state;
            *b = b.wrapping_sub(val as u8);
        });
        let first_buf = fill_first_buf(&decrypt_data).unwrap();
        let (second_buf, result) = fill_second_buf(&first_buf)?;
        let third_buf = fill_third_buf(
            header.unk3 as usize,
            &buf[*off..],
            &second_buf,
            result,
        )?;
        let pixel_data = fill_pixel_data(
            header.width as usize
                * header.height as usize
                * header.bpp as usize
                >> 3,
            &third_buf,
        )?;
        let pixel_data = parse_pixel_data(
            &pixel_data,
            header.width as usize,
            header.height as usize,
            (header.bpp >> 3) as usize,
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

fn prng(mut state: u32) -> (u16, u32) {
    let prev_state = state;
    let mut val = 0x4E35_u32.wrapping_mul(state & 0xFFFF);
    state = (val & 0xFFFF_0000)
        .wrapping_add(0x015A_0000_u32.wrapping_mul(prev_state))
        .wrapping_add(0x4E35_0000_u32.wrapping_mul(prev_state >> 16))
        .wrapping_add(val & 0xFFFF)
        .wrapping_add(1);
    val = (0x15A_u32
        .wrapping_mul(prev_state)
        .wrapping_add(val >> 16)
        .wrapping_sub(0x31CB_u32.wrapping_mul(prev_state >> 16)))
        & 0x7FFF;
    (val as u16, state)
}

fn fill_first_buf(src: &[u8]) -> anyhow::Result<Vec<u8>> {
    let src_index = &mut 0;
    let mut dest = Vec::with_capacity(1024);
    for _ in 0..256 {
        let mut b = 0xFFu8;
        let mut c = 0u32;
        let mut d = 0u32;
        while b >= 0x80 {
            b = src.gread::<u8>(src_index)?;
            let a = (b as u32 & 0x7F) << c;
            c += 7;
            d |= a as u32;
        }
        dest.extend_from_slice(&d.to_le_bytes())
    }
    Ok(dest)
}

fn fill_second_buf(src: &[u8]) -> anyhow::Result<(Vec<u8>, u32)> {
    let mut acc = 0u32;
    let mut dest = vec![0u8; 12264];
    let mut dest_index = 8;
    for i in 0..256u32 {
        let b = src.pread_with::<u32>(i as usize * 4, LE)?;
        dest_index += 24;
        dest[dest_index - 32..dest_index - 28]
            .copy_from_slice(&(if b > 0 { 1u32 } else { 0u32 }).to_le_bytes());
        dest[dest_index - 28..dest_index - 24]
            .copy_from_slice(&b.to_le_bytes());
        dest[dest_index - 24..dest_index - 20]
            .copy_from_slice(&(0u32).to_le_bytes());
        dest[dest_index - 20..dest_index - 16]
            .copy_from_slice(&(0xFFFFFFFFu32).to_le_bytes());
        dest[dest_index - 16..dest_index - 12]
            .copy_from_slice(&i.to_le_bytes());
        dest[dest_index - 12..dest_index - 8].copy_from_slice(&i.to_le_bytes());
        acc = acc.wrapping_add(b);
    }

    for i in 0..255 {
        dest[0x100 * 24 + i * 24..0x100 * 24 + i * 24 + 24].copy_from_slice(
            b"\x00\x00\x00\x00\x00\x00\x00\x00\x01\x00\x00\x00\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF"
        );
    }

    let mut dest_index = 0x1800;
    let mut result = 0x100;
    let mut some_arr = [0, 0];
    loop {
        for i in 0..2 {
            some_arr[i] = 0xFFFFFFFFu32;
            let mut d = 0xFFFFFFFFu32;
            let mut s = 0xFFFFFFFFu32;
            let mut v13 = 0u32;
            if result == 0 {
                return Ok((dest, result));
            }

            let mut index = 0;
            while v13 < result {
                if dest.pread_with::<u32>(index, LE)? != 0 {
                    let b = dest.pread_with::<u32>(index + 4, LE)?;
                    if b < s {
                        d = v13;
                        some_arr[i] = v13;
                        s = b;
                    }
                }
                v13 += 1;
                index += 24;
            }

            if d != 0xFFFFFFFF {
                dest[d as usize * 24..d as usize * 24 + 4]
                    .copy_from_slice(&0u32.to_le_bytes());
                dest[d as usize * 24 + 12..d as usize * 24 + 16]
                    .copy_from_slice(&result.to_le_bytes());
            }
        }
        let s = if some_arr[1] != 0xFFFFFFFF {
            dest.pread_with::<u32>(some_arr[1] as usize * 24 + 4, LE)?
        } else {
            0
        };

        let val = dest
            .pread_with::<u32>(some_arr[0] as usize * 24 + 4, LE)?
            .wrapping_add(s);

        dest[dest_index..dest_index + 4].copy_from_slice(b"\x01\x00\x00\x00");
        dest[dest_index + 4..dest_index + 8]
            .copy_from_slice(&val.to_le_bytes());
        dest[dest_index + 8..dest_index + 16]
            .copy_from_slice(b"\x01\x00\x00\x00\xFF\xFF\xFF\xFF");
        dest[dest_index + 16..dest_index + 20]
            .copy_from_slice(&some_arr[0].to_le_bytes());
        dest[dest_index + 20..dest_index + 24]
            .copy_from_slice(&some_arr[1].to_le_bytes());

        if val == acc {
            return Ok((dest, result));
        }

        result += 1;
        dest_index += 24
    }
}

fn fill_third_buf(
    data_size: usize,
    src: &[u8],
    second_buf: &[u8],
    result: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut src_index = 0;
    let mut dest = vec![0; data_size];
    if data_size == 0 {
        return Ok(dest);
    }
    let some_val =
        second_buf.pread_with::<u32>(result as usize * 24 + 8, LE)?;
    let mut a = 0x80;
    let mut b = result;
    let mut d = some_val;
    for i in 0..data_size {
        if d == 1 {
            loop {
                d = 16;
                if (src[src_index] & a) != 0 {
                    d += 4;
                }
                b = second_buf
                    .pread_with::<u32>(d as usize + b as usize * 24, LE)?;
                a >>= 1;
                if a == 0 {
                    src_index += 1;
                }
                if a == 0 {
                    a = 0x80;
                }
                if second_buf.pread_with::<u32>(b as usize * 24 + 8, LE)? != 1 {
                    break;
                }
            }
            d = some_val;
        }
        dest[i] = b as u8;
        b = result;
    }
    Ok(dest)
}

fn fill_pixel_data(data_size: usize, src: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut dest = vec![0; data_size];
    if src.len() == 0 {
        return Ok(dest);
    }
    let src_index = &mut 0;
    let mut dest_index = 0;
    let mut flag = true;
    while *src_index < src.len() {
        let mut b = 0xFF;
        let mut c = 0;
        let mut d = 0;
        while b >= 0x80 {
            b = src.gread::<u8>(src_index)?;
            let a = ((b & 0x7F) as u32) << c;
            c += 7;
            d |= a as u32;
        }
        if flag {
            dest[dest_index..dest_index + d as usize]
                .copy_from_slice(&src[*src_index..*src_index + d as usize]);
            flag = false;
            *src_index += d as usize;
        } else {
            flag = true;
        }
        dest_index += d as usize;
    }
    Ok(dest)
}

fn parse_pixel_data(
    src: &[u8],
    width: usize,
    height: usize,
    bytes_per_pixel: usize,
) -> anyhow::Result<Vec<u8>> {
    let src_index = &mut 0;
    let mut dest_index = 0;
    let mut dest = vec![0; width * height * 4];
    let mut prev_pixel = [0u8; 4];
    for _ in 0..width {
        let p = &src[*src_index..*src_index + bytes_per_pixel];
        *src_index += bytes_per_pixel;
        for i in 0..bytes_per_pixel {
            prev_pixel[i] = prev_pixel[i].wrapping_add(p[i]);
        }
        dest[dest_index..dest_index + 4].copy_from_slice(&prev_pixel);
        dest_index += 4;
    }

    let mut prev_line_index = 0;
    for _ in 0..height - 1 {
        let p = &src[*src_index..*src_index + bytes_per_pixel];
        let mut x: [u8; 4] =
            dest[prev_line_index..prev_line_index + 4].try_into()?;
        *src_index += bytes_per_pixel;
        prev_line_index += 4;
        for i in 0..bytes_per_pixel {
            x[i] = x[i].wrapping_add(p[i]);
        }
        dest[dest_index..dest_index + 4].copy_from_slice(&x);
        dest_index += 4;
        let mut x = punpcklbw0(&x);
        for _ in 0..width - 1 {
            let mut p2 = [0u8; 4];
            p2[..bytes_per_pixel].copy_from_slice(
                &src[*src_index..*src_index + bytes_per_pixel],
            );
            let x2 = &dest[prev_line_index..prev_line_index + 4];
            prev_line_index += 4;
            *src_index += bytes_per_pixel;
            let x2 = punpcklbw0(&x2);
            for i in 0..4 {
                let v =
                    x[i * 2..i * 2 + 2].pread_with::<u16>(0, LE)?.wrapping_add(
                        x2[i * 2..i * 2 + 2].pread_with::<u16>(0, LE)?,
                    ) >> 1;
                x[i * 2..i * 2 + 2].copy_from_slice(&v.to_le_bytes());
            }
            let p2 = punpcklbw0(&p2);
            for i in 0..8 {
                x[i] = x[i].wrapping_add(p2[i]);
            }
            dest[dest_index..dest_index + 4]
                .copy_from_slice(&packuswb(&x)?.to_be_bytes());
            dest_index += 4;
        }
    }

    if bytes_per_pixel == 3 {
        dest.chunks_exact_mut(4).for_each(|c| c[3] = 0xFF);
    }

    Ok(dest)
}

fn punpcklbw0(xmm0: &[u8]) -> [u8; 8] {
    let mut dest = [0; 8];
    for i in 0..4 {
        dest[i * 2] = xmm0[i];
    }
    dest
}

fn packuswb(xmm0: &[u8]) -> anyhow::Result<u32> {
    let mut result = 0u32;
    for i in 0..4 {
        result <<= 8;
        let b = xmm0.pread_with::<i16>(i * 2, LE)?;
        result |= if b > 0xFF {
            0xFF
        } else if b < 0 {
            0
        } else {
            (b & 0xFF) as u32
        };
    }
    Ok(result)
}
