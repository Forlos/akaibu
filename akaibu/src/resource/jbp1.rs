use bytes::Bytes;
use scroll::{Pread, LE};

struct Jbp1 {
    data_offset: u32,
    flags: u32,
    width: u16,
    height: u16,
    depth: u16,
    bit_pool_size_1: u32,
    bit_pool_size_2: u32,
    blocks_width: u16,
    blocks_height: u16,
    block_stride: u16,
    x_block_count: u16,
    y_block_count: u16,
    x_block_size: u16,
    y_block_size: u16,
}

impl Jbp1 {
    fn new(buf: &[u8]) -> anyhow::Result<Self> {
        let off = &mut 0;
        let data_offset = buf.gread_with::<u32>(off, LE)?;
        let flags = buf.gread_with::<u32>(off, LE)?;
        *off += 4;
        let width = buf.gread_with::<u16>(off, LE)?;
        let height = buf.gread_with::<u16>(off, LE)?;
        let depth = buf.gread_with::<u16>(off, LE)?;
        *off += 6;
        let bit_pool_size_1 = buf.gread_with::<u32>(off, LE)?;
        let bit_pool_size_2 = buf.gread_with::<u32>(off, LE)?;

        let x_block_size;
        let y_block_size;

        match flags >> 28 & 3 {
            0 => {
                x_block_size = 8;
                y_block_size = 8;
            }
            1 => {
                x_block_size = 16;
                y_block_size = 16;
            }
            2 => {
                x_block_size = 32;
                y_block_size = 16;
            }
            _ => unimplemented!(),
        }

        let blocks_width = (width + x_block_size - 1) & !(x_block_size - 1);
        let blocks_height = (height + y_block_size - 1) & !(y_block_size - 1);
        let block_stride = blocks_width * 4;
        let x_block_count = blocks_width >> 4;
        let y_block_count = blocks_height >> 4;

        Self {
            data_offset,
            flags,
            width,
            height,
            depth,
            bit_pool_size_1,
            bit_pool_size_2,
            blocks_width,
            blocks_height,
            block_stride,
            x_block_count,
            y_block_count,
            x_block_size,
            y_block_size,
        }
    }
}

fn jbp1_decompress(buf: &[u8]) -> anyhow::Result<Bytes> {
    let off = &mut 0;
    let jbp1 = Jbp1::new(buf)?;
    *off = jbp1.data_offset as usize;
    let freq_dc = vec![0; 128];
    for i in 0..16 {
        freq_dc[i] = buf.gread_with::<u32>(off, LE)?;
    }
    let freq_ac = vec![0; 128];
    for i in 0..16 {
        freq_ac[i] = buf.gread_with::<u32>(off, LE)?;
    }

    todo!()
}
