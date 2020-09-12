use crate::error::AkaibuError;
use image::{buffer::ConvertBuffer, ImageBuffer, RgbaImage};
use scroll::{Pread, LE};

#[derive(Debug)]
pub(crate) struct Pb3b {
    header: Header,
    pub(crate) image: RgbaImage,
}

impl Pb3b {
    pub(crate) fn from_bytes(mut buf: Vec<u8>) -> anyhow::Result<Self> {
        Self::decrypt(&mut buf);
        let header = buf.pread_with::<Header>(0x18, LE)?;
        // if header.main_type == 1
        //     || header.main_type == 5
        //     || header.main_type == 6
        // {
        //     return Err(AkaibuError::Unimplemented.into());
        // }
        let image = match header.main_type {
            1 => Self::decode_v1(&mut buf, &header),
            // 3 => Self::decode_v3(&mut buf, &header),
            5 => Self::decode_v5(&mut buf, &header),
            6 => Self::decode_v6(&mut buf, &header),
            _ => return Err(AkaibuError::Unimplemented.into()),
        }?;
        Ok(Self { header, image })
    }
    fn decrypt(buf: &mut [u8]) {
        let tail_key = &buf[buf.len() - 0x2F..buf.len() - 3].to_vec();
        let pair_key = &buf[buf.len() - 3..buf.len() - 1].to_vec();
        buf.iter_mut()
            .skip(8)
            .take(0x2C)
            .enumerate()
            .for_each(|(i, b)| {
                *b ^= pair_key[i % pair_key.len()];
                *b = b.wrapping_sub(tail_key[i]);
            });
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
                main_offsets.last().unwrap() + main_sizes[channel - 1] as usize,
            );
            data_offsets.push(
                data_offsets.last().unwrap() + data_sizes[channel - 1] as usize,
            );
        }

        for channel in 0..channel_count {
            *off = main_offsets[channel];
            let control_block1_size = buf.gread_with::<u32>(off, LE)? as usize;
            let data_block1_size = buf.gread_with::<u32>(off, LE)? as usize;
            let size_orig = buf.gread_with::<u32>(off, LE)? as usize;

            let control_block1 = &buf[*off..*off + control_block1_size];
            *off += control_block1_size;
            let data_block1 = &buf[*off..*off + data_block1_size];
            *off += data_block1_size;
            let control_block2 =
                &buf[*off..*off + main_offsets[channel] + main_sizes[channel]];
            *off = data_offsets[channel];
            let data_block2 = &buf[*off..*off + data_sizes[channel]];

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
        // let mut image: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
        //     ImageBuffer::new(header.width as u32, header.height as u32);
        // let jbp1_data = &buf[0x34..];
        todo!()
        // Ok(image.convert())
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
            control_sizes.push(control_offsets[i] - control_offsets[i - 1]);
            data_sizes.push(data_offsets[i] - data_offsets[i - 1]);
        }
        control_sizes.push(buf.len() - control_offsets.last().unwrap());
        data_sizes.push(buf.len() - data_offsets.last().unwrap());

        for channel in 0..channel_count {
            let control_block = &buf[control_offsets[channel]
                ..control_offsets[channel] + control_sizes[channel]];
            let data_block = &buf[data_offsets[channel]
                ..data_offsets[channel] + data_sizes[channel]];
            let plane = Self::custom_lzss(
                control_block,
                data_block,
                header.width as usize * header.height as usize,
            )?;
            let plane_off = &mut 0;
            let mut acc = 0u8;
            for y in 0..header.height {
                for x in 0..header.width {
                    acc = acc.wrapping_add(plane[*plane_off]);
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

        let control_block1 = &buf
            [control_block_offset..control_block_offset + control_block_size];
        let data_block1 =
            &buf[data_block_offset..data_block_offset + data_block_size];
        let proxy_block =
            Self::custom_lzss(control_block1, data_block1, size_orig)?;

        let proxy_off = &mut 0;
        let control_block2_size =
            proxy_block.gread_with::<u32>(proxy_off, LE)? as usize;
        let data_block2_size =
            proxy_block.gread_with::<u32>(proxy_off, LE)? as usize;
        let control_block2 =
            &proxy_block[*proxy_off..*proxy_off + control_block2_size];
        *proxy_off += control_block2_size;
        let data_block2 =
            &proxy_block[*proxy_off..*proxy_off + data_block2_size];

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
                    let b = dict[src_ptr];
                    src_ptr = (src_ptr + 1) % dict.len();

                    output[i] = b;
                    i += 1;

                    dict[*dict_off] = b;
                    *dict_off = (*dict_off + 1) % dict.len();

                    repetitions -= 1;
                }
            } else {
                let b = data_block.gread(data_off)?;
                output[i] = b;
                i += 1;
                dict[*dict_off] = b;
                *dict_off = (*dict_off + 1) % dict.len();
            }
            bit_mask >>= 1;
        }

        Ok(output)
    }
}

#[derive(Debug, Pread)]
struct Header {
    sub_type: u32,
    main_type: u16,
    width: u16,
    height: u16,
    depth: u16,
}
