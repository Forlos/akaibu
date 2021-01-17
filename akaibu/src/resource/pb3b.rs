use super::{jbp1::jbp1_decompress, ResourceScheme, ResourceType};
use crate::error::AkaibuError;
use anyhow::Context;
use image::{buffer::ConvertBuffer, ImageBuffer, RgbaImage};
use scroll::{Pread, LE};
use std::{fs::File, io::Read, path::PathBuf};

#[derive(Debug, Clone)]
pub(crate) enum Pb3bScheme {
    Universal,
}

#[derive(Debug, Pread)]
struct Header {
    sub_type: u32,
    main_type: u16,
    width: u16,
    height: u16,
    depth: u16,
}

impl ResourceScheme for Pb3bScheme {
    fn convert(&self, file_path: &PathBuf) -> anyhow::Result<ResourceType> {
        let mut buf = Vec::with_capacity(1 << 20);
        let mut file = File::open(file_path)?;
        file.read_to_end(&mut buf)?;
        self.from_bytes(buf)
    }
    fn convert_from_bytes(
        &self,
        _file_path: &PathBuf,
        buf: Vec<u8>,
    ) -> anyhow::Result<ResourceType> {
        self.from_bytes(buf)
    }
    fn get_name(&self) -> String {
        format!(
            "[PB3B] {}",
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

impl Pb3bScheme {
    fn from_bytes(&self, mut buf: Vec<u8>) -> anyhow::Result<ResourceType> {
        Self::decrypt(&mut buf)?;
        let header = buf.pread_with::<Header>(0x18, LE)?;
        let image = match header.main_type {
            1 => Self::decode_v1(&mut buf, &header),
            2 | 3 => Self::decode_v3(&mut buf, &header),
            5 => Self::decode_v5(&mut buf, &header),
            6 => Self::decode_v6(&mut buf, &header),
            _ => {
                return Err(AkaibuError::Unimplemented(format!(
                    "PB3 version {} is not supported",
                    header.main_type
                ))
                .into())
            }
        }?;
        Ok(ResourceType::RgbaImage { image })
    }
    fn decrypt(buf: &mut [u8]) -> anyhow::Result<()> {
        let tail_key = &buf
            .get(buf.len() - 0x2F..buf.len() - 3)
            .context("Out of bounds access")?
            .to_vec();
        let pair_key = &buf
            .get(buf.len() - 3..buf.len() - 1)
            .context("Out of bounds access")?
            .to_vec();
        buf.iter_mut()
            .skip(8)
            .take(0x2C)
            .enumerate()
            .try_for_each(|(i, b)| {
                *b ^= pair_key
                    .get(i % pair_key.len())
                    .context("Out of bounds access")?;
                *b = b.wrapping_sub(
                    *tail_key.get(i).context("Out of bounds access")?,
                );
                Ok(())
            })
    }
    fn decode_v1(buf: &mut [u8], header: &Header) -> anyhow::Result<RgbaImage> {
        let off = &mut 0x2C;
        let mut image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
            ImageBuffer::new(header.width as u32, header.height as u32);

        let channel_count = (header.depth >> 3) as usize;

        let main_sizes_offset = buf.gread_with::<u32>(off, LE)? as usize;
        let data_sizes_offset = buf.gread_with::<u32>(off, LE)? as usize;

        *off = main_sizes_offset;
        let mut main_sizes = Vec::with_capacity(channel_count);
        for _ in 0..channel_count {
            main_sizes.push(buf.gread_with::<u32>(off, LE)? as usize);
        }

        *off = data_sizes_offset;
        let mut data_sizes = Vec::with_capacity(channel_count);
        for _ in 0..channel_count {
            data_sizes.push(buf.gread_with::<u32>(off, LE)? as usize);
        }

        let mut main_offsets = Vec::new();
        main_offsets.push(main_sizes_offset + 4 * channel_count);
        let mut data_offsets = Vec::new();
        data_offsets.push(data_sizes_offset + 4 * channel_count);
        for channel in 1..channel_count {
            main_offsets.push(
                main_offsets
                    .last()
                    .context("Could not get last main_offset")?
                    + *main_sizes
                        .get(channel - 1)
                        .context("Out of bounds access")?,
            );
            data_offsets.push(
                data_offsets
                    .last()
                    .context("Could not get last data_offset")?
                    + *data_sizes
                        .get(channel - 1)
                        .context("Out of bounds access")?,
            );
        }

        for channel in 0..channel_count {
            *off =
                *main_offsets.get(channel).context("Out of bounds access")?;
            let control_block1_size = buf.gread_with::<u32>(off, LE)? as usize;
            let data_block1_size = buf.gread_with::<u32>(off, LE)? as usize;
            let size_orig = buf.gread_with::<u32>(off, LE)? as usize;

            let control_block1 = buf
                .get(*off..*off + control_block1_size)
                .context("Out of bounds access")?;
            *off += control_block1_size;
            let data_block1 = buf
                .get(*off..*off + data_block1_size)
                .context("Out of bounds access")?;
            *off += data_block1_size;
            let main_offset =
                *main_offsets.get(channel).context("Out of bounds access")?;
            let main_size =
                *main_sizes.get(channel).context("Out of bounds access")?;
            let data_offset =
                *data_offsets.get(channel).context("Out of bounds access")?;
            let data_size =
                data_sizes.get(channel).context("Out of bounds access")?;
            let control_block2 = if (*off + main_offset + main_size) > buf.len()
            {
                buf.get(*off..buf.len()).context("Out of bounds access")?
            } else {
                buf.get(*off..*off + main_offset + main_size)
                    .context("Out of bounds access")?
            };
            *off = data_offset;
            let data_block2 = &buf
                .get(*off..*off + data_size)
                .context("Out of bounds access")?;

            let plane =
                Self::custom_lzss(control_block2, data_block2, size_orig)?;

            let block_size = 16;
            let mut x_block_count = header.width / block_size;
            let mut y_block_count = header.height / block_size;
            if header.width % block_size > 0 {
                x_block_count += 1;
            }
            if header.height % block_size > 0 {
                y_block_count += 1;
            }
            let mut bit_mask = 0;
            let mut control = 0;

            let control_off = &mut 0;
            let data_off = &mut 0;
            let plane_off = &mut 0;
            for block_y in 0..y_block_count {
                for block_x in 0..x_block_count {
                    let block_x1 = (block_x * block_size) as u32;
                    let block_y1 = (block_y * block_size) as u32;
                    let block_x2 = std::cmp::min(
                        block_x1 + block_size as u32,
                        header.width as u32,
                    );
                    let block_y2 = std::cmp::min(
                        block_y1 + block_size as u32,
                        header.height as u32,
                    );

                    if bit_mask == 0 {
                        control = control_block1.gread::<u8>(control_off)?;
                        bit_mask = 0x80;
                    }

                    if control & bit_mask != 0 {
                        let b = data_block1.gread::<u8>(data_off)?;
                        for y in block_y1..block_y2 {
                            for x in block_x1..block_x2 {
                                image.get_pixel_mut(x, y)[channel] = b;
                            }
                        }
                    } else {
                        for y in block_y1..block_y2 {
                            for x in block_x1..block_x2 {
                                image.get_pixel_mut(x, y)[channel] =
                                    plane.gread::<u8>(plane_off)?;
                            }
                        }
                    }
                    bit_mask >>= 1;
                }
            }
        }

        if header.depth != 32 {
            for p in image.pixels_mut() {
                p[3] = 0xFF;
            }
        }

        Ok(image.convert())
    }
    fn decode_v3(buf: &mut [u8], header: &Header) -> anyhow::Result<RgbaImage> {
        let jbp1_data = buf.get(0x34..).context("Out of bounds access")?;
        let mut output = jbp1_decompress(jbp1_data)?;
        let mut alpha_pos = buf.pread_with::<u32>(0x2C, LE)? as usize;
        if header.depth == 32 && alpha_pos != 0 {
            let mut dst = 3;
            let end = header.width as usize * header.height as usize * 4;
            while dst < end {
                let alpha =
                    *buf.get(alpha_pos).context("Out of bounds access")?;
                alpha_pos += 1;
                if alpha != 0 && alpha != 255 {
                    output[dst] = alpha;
                    dst += 4;
                } else {
                    let mut count =
                        *buf.get(alpha_pos).context("Out of bounds access")?;
                    alpha_pos += 1;
                    while count > 0 {
                        *output
                            .get_mut(dst)
                            .context("Out of bounds access")? = alpha;
                        dst += 4;
                        count -= 1;
                    }
                }
            }
        }
        let image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
            ImageBuffer::from_vec(
                header.width as u32,
                header.height as u32,
                output,
            )
            .context("Invalid image resolution")?;
        Ok(image.convert())
    }

    fn decode_v5(buf: &mut [u8], header: &Header) -> anyhow::Result<RgbaImage> {
        let off = &mut 0x34;
        let mut image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
            ImageBuffer::new(header.width as u32, header.height as u32);
        let channel_count = (header.depth >> 3) as usize;

        let mut control_offsets = Vec::with_capacity(channel_count);
        let mut data_offsets = Vec::with_capacity(channel_count);
        for _ in 0..channel_count {
            control_offsets
                .push(0x54 + buf.gread_with::<u32>(off, LE)? as usize);
            data_offsets.push(0x54 + buf.gread_with::<u32>(off, LE)? as usize);
        }

        let mut control_sizes = Vec::with_capacity(channel_count);
        let mut data_sizes = Vec::with_capacity(channel_count);
        for i in 1..channel_count {
            control_sizes.push(
                *control_offsets.get(i).context("Out of bounds access")?
                    - *control_offsets
                        .get(i - 1)
                        .context("Out of bounds access")?,
            );
            data_sizes.push(
                *data_offsets.get(i).context("Out of bounds access")?
                    - *data_offsets
                        .get(i - 1)
                        .context("Out of bounds access")?,
            );
        }
        control_sizes.push(
            buf.len()
                - control_offsets
                    .last()
                    .context("Could not get last control_offset")?,
        );
        data_sizes.push(
            buf.len()
                - data_offsets
                    .last()
                    .context("Could not get last data_offset")?,
        );

        for channel in 0..channel_count {
            let control_block = buf
                .get(
                    control_offsets[channel]
                        ..control_offsets[channel] + control_sizes[channel],
                )
                .context("Out of bounds access")?;
            let data_block = buf
                .get(
                    data_offsets[channel]
                        ..data_offsets[channel] + data_sizes[channel],
                )
                .context("Out of bounds access")?;
            let plane = Self::custom_lzss(
                control_block,
                data_block,
                header.width as usize * header.height as usize,
            )?;
            let plane_off = &mut 0;
            let mut acc = 0u8;
            for y in 0..header.height {
                for x in 0..header.width {
                    acc = acc.wrapping_add(
                        *plane
                            .get(*plane_off)
                            .context("Out of bounds access")?,
                    );
                    *plane_off += 1;
                    image.get_pixel_mut(x as u32, y as u32)[channel] = acc;
                }
            }
        }

        Ok(image.convert())
    }
    fn decode_v6(buf: &mut [u8], header: &Header) -> anyhow::Result<RgbaImage> {
        let mut image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
            ImageBuffer::new(header.width as u32, header.height as u32);

        let size_orig = buf.pread_with::<u32>(0x18, LE)? as usize;
        let control_block_offset =
            0x20 + buf.pread_with::<u32>(0xC, LE)? as usize;
        let data_block_offset =
            control_block_offset + buf.pread_with::<u32>(0x2C, LE)? as usize;
        let data_block_size = buf.pread_with::<u32>(0x30, LE)? as usize;
        let control_block_size = data_block_offset - control_block_offset;

        let control_block1 = buf
            .get(
                control_block_offset..control_block_offset + control_block_size,
            )
            .context("Out of bounds access")?;
        let data_block1 = buf
            .get(data_block_offset..data_block_offset + data_block_size)
            .context("Out of bounds access")?;
        let proxy_block =
            Self::custom_lzss(control_block1, data_block1, size_orig)?;

        let proxy_off = &mut 0;
        let control_block2_size =
            proxy_block.gread_with::<u32>(proxy_off, LE)? as usize;
        let data_block2_size =
            proxy_block.gread_with::<u32>(proxy_off, LE)? as usize;
        let control_block2 = &proxy_block
            .get(*proxy_off..*proxy_off + control_block2_size)
            .context("Out of bounds access")?;
        *proxy_off += control_block2_size;
        let data_block2 = &proxy_block
            .get(*proxy_off..*proxy_off + data_block2_size)
            .context("Out of bounds access")?;

        let control_off = &mut 0;
        let data_off = &mut 0;

        let block_size = 8;
        let mut bit_mask = 0;
        let mut control = 0;
        let mut x_block_count = header.width / block_size;
        let mut y_block_count = header.height / block_size;
        if header.width % block_size != 0 {
            x_block_count += 1;
        }
        if header.height % block_size != 0 {
            y_block_count += 1;
        }

        for block_y in 0..y_block_count {
            for block_x in 0..x_block_count {
                let block_x1 = block_x * block_size;
                let block_y1 = block_y * block_size;
                let block_x2 =
                    std::cmp::min(block_x1 + block_size, header.width);
                let block_y2 =
                    std::cmp::min(block_y1 + block_size, header.height);

                if bit_mask == 0 {
                    control = control_block2.gread::<u8>(control_off)?;
                    bit_mask = 0x80;
                }
                if (control & bit_mask) == 0 {
                    for y in block_y1..block_y2 {
                        for x in block_x1..block_x2 {
                            let pixel: [u8; 4] = data_block2
                                .gread_with::<u32>(data_off, LE)?
                                .to_le_bytes();
                            *image.get_pixel_mut(x as u32, y as u32) =
                                image::Bgra::from(pixel);
                        }
                    }
                }
                bit_mask >>= 1;
            }
        }

        Ok(image.convert())
    }
    fn custom_lzss(
        control_block: &[u8],
        data_block: &[u8],
        output_size: usize,
    ) -> anyhow::Result<Vec<u8>> {
        let control_off = &mut 0;
        let data_off = &mut 0;
        let dict_off = &mut 0x7DE;
        let mut dict = vec![0; 0x800];
        let mut output = vec![0; output_size];

        let mut bit_mask = 0;
        let mut control = 0;

        let mut i = 0;
        while i < output.len() {
            if bit_mask == 0 {
                bit_mask = 0x80;
                control = control_block.gread::<u8>(control_off)?;
            }
            if (control & bit_mask) > 0 {
                let tmp = data_block.gread_with::<u16>(data_off, LE)?;
                let look_behind_pos = tmp >> 5;
                let mut src_ptr = look_behind_pos as usize;
                let mut repetitions = (tmp & 0x1F) + 3;
                while repetitions > 0 && i < output.len() {
                    let b =
                        *dict.get(src_ptr).context("Out of bounds access")?;
                    src_ptr = (src_ptr + 1) % dict.len();

                    *output.get_mut(i).context("Out of bounds access")? = b;
                    i += 1;

                    *dict
                        .get_mut(*dict_off)
                        .context("Out of bounds access")? = b;
                    *dict_off = (*dict_off + 1) % dict.len();

                    repetitions -= 1;
                }
            } else {
                let b = data_block.gread(data_off)?;
                *output.get_mut(i).context("Out of bounds access")? = b;
                i += 1;
                *dict.get_mut(*dict_off).context("Out of bounds access")? = b;
                *dict_off = (*dict_off + 1) % dict.len();
            }
            bit_mask >>= 1;
        }

        Ok(output)
    }
}
