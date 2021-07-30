use crate::{archive, error::AkaibuError, util::image::remove_bitmap_padding};

use super::{ResourceScheme, ResourceType};
use anyhow::Context;
use image::{buffer::ConvertBuffer, GrayImage, ImageBuffer};
use scroll::Pread;
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, Clone)]
pub(crate) enum IarScheme {
    Universal,
}

#[derive(Debug, Pread)]
struct IarHeader {
    version: u32,
    unk0: u32,
    decompressed_file_size: u32,
    unk1: u32,
    file_size: u32,
    unk2: u32,
    unk3: u32,
    unk4: u32,
    width: u32,
    height: u32,
    unknown: [u8; 32],
}

impl ResourceScheme for IarScheme {
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
            "[IAR] {}",
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

impl IarScheme {
    fn from_bytes(&self, buf: Vec<u8>) -> anyhow::Result<ResourceType> {
        let header = buf.pread::<IarHeader>(0)?;
        let data = if header.version >> 24 == 1 {
            decompress(&buf[72..], header.decompressed_file_size as usize)?
        } else {
            buf[72..].to_vec()
        };
        match header.version & 0xFFFF {
            0x3C => {
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
            0x1C => {
                let data = remove_bitmap_padding(
                    data,
                    header.decompressed_file_size as usize
                        / header.height as usize,
                    calculate_padding(header.width),
                );
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
            0x2 => {
                let image: GrayImage = ImageBuffer::from_vec(
                    header.width as u32,
                    header.height as u32,
                    data,
                )
                .context("Invalid image resolution")?;
                Ok(ResourceType::RgbaImage {
                    image: image.convert(),
                })
            }
            ver => Err(AkaibuError::Custom(format!(
                "Unsupported version {:X}",
                ver
            ))
            .into()),
        }
    }
}

fn calculate_padding(width: u32) -> usize {
    let padding = 4 - ((width as usize * 3) % 4);
    if padding == 4 {
        0
    } else {
        padding
    }
}

fn decompress(src: &[u8], dest_len: usize) -> anyhow::Result<Vec<u8>> {
    let mut src_index = 0;
    let mut dest_index = 0;
    let mut dest = vec![0; dest_len];
    let mut counter = 0u32;
    let mut s;
    let mut b;
    let mut var_c;
    loop {
        'inner: loop {
            counter >>= 1;
            if counter <= 0xFFFF {
                counter = src[src_index] as u32
                    | ((src[src_index + 1] as u32 | 0xFFFF_FF00) << 8);
                src_index += 2;
            }
            if counter & 1 == 0 {
                break 'inner;
            }
            dest[dest_index] = src[src_index];
            src_index += 1;
            dest_index += 1;
        }

        counter >>= 1;
        if counter <= 0xFFFF {
            counter = src[src_index] as u32
                | ((src[src_index + 1] as u32 | 0xFFFF_FF00) << 8);
            src_index += 2;
        }

        if counter & 1 == 0 {
            counter >>= 1;
            b = 2;
            var_c = b;
            if counter <= 0xFFFF {
                counter = src[src_index] as u32
                    | ((src[src_index + 1] as u32 | 0xFFFF_FF00) << 8);
                src_index += 2;
            }

            if counter & 1 == 0 {
                s = src[src_index] as u32 + 1;
                src_index += 1;
                if s == 256 {
                    return Ok(dest);
                }
            } else {
                counter >>= 1;
                if counter <= 0xFFFF {
                    counter = src[src_index] as u32
                        | ((src[src_index + 1] as u32 | 0xFFFF_FF00) << 8);
                    src_index += 2;
                }
                let mut d = (counter & 1) << 10;
                counter >>= 1;
                if counter <= 0xFFFF {
                    counter = src[src_index] as u32
                        | ((src[src_index + 1] as u32 | 0xFFFF_FF00) << 8);
                    src_index += 2;
                }
                let a = (counter & 1) << 9;
                counter >>= 1;
                d |= a;
                if counter <= 0xFFFF {
                    counter = src[src_index] as u32
                        | ((src[src_index + 1] as u32 | 0xFFFF_FF00) << 8);
                    src_index += 2;
                }
                s = ((((counter & 1) << 8) | src[src_index] as u32) | d)
                    .wrapping_add(256);
                src_index += 1;
            }
        } else {
            counter >>= 1;
            let mut d = 1;
            if counter <= 0xFFFF {
                counter = src[src_index] as u32
                    | ((src[src_index + 1] as u32 | 0xFFFF_FF00) << 8);
                src_index += 2;
            }
            s = counter;
            counter >>= 1;
            s &= d;
            if counter <= 0xFFFF {
                counter = src[src_index] as u32
                    | ((src[src_index + 1] as u32 | 0xFFFF_FF00) << 8);
                src_index += 2;
            }
            if counter & 1 == 0 {
                counter >>= 1;
                d = 513;
                if counter <= 0xFFFF {
                    counter = src[src_index] as u32
                        | ((src[src_index + 1] as u32 | 0xFFFF_FF00) << 8);
                    src_index += 2;
                }
                if counter & 1 == 0 {
                    counter >>= 1;
                    d = 1025;
                    if counter <= 0xFFFF {
                        counter = src[src_index] as u32
                            | ((src[src_index + 1] as u32 | 0xFFFF_FF00) << 8);
                        src_index += 2;
                    }
                    let mut a = counter & 1;
                    counter >>= 1;
                    s = s.wrapping_add(s);
                    s |= a;
                    if counter <= 0xFFFF {
                        counter = src[src_index] as u32
                            | ((src[src_index + 1] as u32 | 0xFFFF_FF00) << 8);
                        src_index += 2;
                    }
                    if counter & 1 == 0 {
                        counter >>= 1;
                        d = 2049;
                        if counter <= 0xFFFF {
                            counter = src[src_index] as u32
                                | ((src[src_index + 1] as u32 | 0xFFFF_FF00)
                                    << 8);
                            src_index += 2;
                        }
                        a = counter & 1;
                        counter >>= 1;
                        s = s.wrapping_add(s);
                        s |= a;
                        if counter <= 0xFFFF {
                            counter = src[src_index] as u32
                                | ((src[src_index + 1] as u32 | 0xFFFF_FF00)
                                    << 8);
                            src_index += 2;
                        }
                        if counter & 1 == 0 {
                            counter >>= 1;
                            d = 4097;
                            if counter <= 0xFFFF {
                                counter = src[src_index] as u32
                                    | ((src[src_index + 1] as u32
                                        | 0xFFFF_FF00)
                                        << 8);
                                src_index += 2;
                            }
                            s = s.wrapping_add(s);
                            s |= counter & 1;
                        }
                    }
                }
            }
            s = (s << 8) | src[src_index] as u32;
            src_index += 1;
            counter >>= 1;
            s = s.wrapping_add(d);
            let mut var_4 = src_index;
            if counter <= 0xFFFF {
                counter = src[src_index] as u32
                    | ((src[src_index + 1] as u32 | 0xFFFF_FF00) << 8);
                src_index += 2;
                var_4 = src_index;
            }

            b = 3;
            if counter & 1 == 0 {
                counter >>= 1;
                if counter <= 0xFFFF {
                    counter = src[src_index] as u32
                        | ((src[src_index + 1] as u32 | 0xFFFF_FF00) << 8);
                    src_index += 2;
                    var_4 = src_index;
                }
                b = 4;
                if counter & 1 == 0 {
                    counter >>= 1;
                    if counter <= 0xFFFF {
                        counter = src[src_index] as u32
                            | ((src[src_index + 1] as u32 | 0xFFFF_FF00) << 8);
                        src_index += 2;
                        var_4 = src_index;
                    }
                    b = 5;
                    if counter & 1 == 0 {
                        counter >>= 1;
                        if counter <= 0xFFFF {
                            counter = src[src_index] as u32
                                | ((src[src_index + 1] as u32 | 0xFFFF_FF00)
                                    << 8);
                            src_index += 2;
                            var_4 = src_index;
                        }
                        b = 6;
                        if counter & 1 == 0 {
                            counter >>= 1;
                            let mut var_8 = counter;
                            if counter <= 0xFFFF {
                                counter = src[src_index] as u32
                                    | ((src[src_index + 1] as u32
                                        | 0xFFFF_FF00)
                                        << 8);
                                src_index += 2;
                                var_8 = counter;
                                var_4 = src_index;
                            }
                            if counter & 1 == 0 {
                                let (a, second, third) =
                                    some_fn(var_4, var_8, src);
                                var_4 = second;
                                var_8 = third;
                                if a == 0 {
                                    let (mut d, second, third) =
                                        some_fn(second, third, src);
                                    d <<= 2;
                                    let (mut a, second, third) =
                                        some_fn(second, third, src);
                                    a = a.wrapping_add(a);
                                    d |= a;
                                    let (a, second, third) =
                                        some_fn(second, third, src);
                                    var_4 = second;
                                    var_8 = third;
                                    src_index = var_4;
                                    b = a | d;
                                    b = b.wrapping_add(9);
                                } else {
                                    src_index = var_4 + 1;
                                    b = src[src_index - 1] as u32 + 17;
                                }
                            } else {
                                let (a, second, third) =
                                    some_fn(var_4, var_8, src);
                                var_4 = second;
                                var_8 = third;
                                src_index = var_4;
                                if a == 0 {
                                    b = 7;
                                } else {
                                    b = 8;
                                }
                            }
                            counter = var_8;
                        }
                    }
                }
            }
            var_c = b;
        }

        let mut d = dest_index;
        d -= s as usize;
        for _ in 0..b {
            dest[d + s as usize] = dest[d];
            d += 1;
        }
        if b != 0 {
            b = var_c;
        }
        dest_index += b as usize;
    }
}

fn some_fn(mut var_4: usize, mut var_8: u32, src: &[u8]) -> (u32, usize, u32) {
    var_8 >>= 1;
    if var_8 <= 0xFFFF {
        let s =
            src[var_4] as u32 | ((src[var_4 + 1] as u32 | 0xFFFF_FF00) << 8);
        var_8 = s;
        var_4 += 2;
    }
    (var_8 & 1, var_4, var_8)
}
