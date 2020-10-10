use std::iter;

use anyhow::Context;
use once_cell::sync::Lazy;
use scroll::{Pread, LE};

static LOOKUP_TABLE: Lazy<Vec<u8>> = Lazy::new(|| {
    iter::repeat(0)
        .take(256)
        .chain(0..=255)
        .chain(iter::repeat(255).take(256))
        .collect::<Vec<u8>>()
});

struct Jbp1 {
    data_offset: u32,
    flags: u32,
    depth: u16,
    bit_pool_size_1: u32,
    bit_pool_size_2: u32,
    blocks_width: u16,
    blocks_height: u16,
    block_stride: u16,
    x_block_count: u16,
    y_block_count: u16,
}

impl Jbp1 {
    fn new(buf: &[u8]) -> anyhow::Result<Self> {
        let off = &mut 4;
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

        Ok(Self {
            data_offset,
            flags,
            depth,
            bit_pool_size_1,
            bit_pool_size_2,
            blocks_width,
            blocks_height,
            block_stride,
            x_block_count,
            y_block_count,
        })
    }
}

struct BitStream {
    buf: Vec<u8>,
    buffer: u8,
    bits_available: u8,
    off: usize,
}

impl BitStream {
    fn new(buf: Vec<u8>) -> Self {
        Self {
            buf,
            buffer: 0,
            bits_available: 0,
            off: 0,
        }
    }
    fn read(&mut self, bits: usize) -> anyhow::Result<u32> {
        let mut ret: u32 = 0;
        for _ in 0..bits {
            if self.bits_available == 0 {
                self.buffer = self.buf.gread(&mut self.off)?;
                self.bits_available = 8;
            }
            ret <<= 1;
            ret |= (self.buffer & 1) as u32;
            self.buffer >>= 1;
            self.bits_available -= 1;
        }
        Ok(ret)
    }
}

struct Tree {
    neighbour: Vec<u32>,
    root: usize,
    input_size: usize,
}

impl Tree {
    fn new(input: &[u8], freq: &mut [u32]) -> Self {
        let mut neighbour: Vec<u32> = vec![0; 1024];
        let mut other: Vec<u32> = vec![0; 258];
        let max = 2100000000;
        let mut size = input.len();
        let mut c = !size + 1;
        let mut idx = size + 512;
        loop {
            let mut d: i64 = -1;
            let mut n: i64 = -1;
            {
                let mut x = max - 1;
                for (i, val) in freq.iter().enumerate().take(size) {
                    if (freq[i] as usize) < x {
                        n = i as i64;
                        x = *val as usize;
                    }
                }
            }

            {
                let mut x = max - 1;
                for (i, val) in freq.iter().enumerate().take(size) {
                    if (i as i64 != n) && (freq[i] as usize) < x {
                        d = i as i64;
                        x = *val as usize;
                    }
                }
            }

            if n < 0 || d < 0 {
                break;
            }

            neighbour[idx - 512] = n as u32;
            neighbour[idx] = d as u32;
            idx += 1;

            other[n as usize] = size as u32;
            other[d as usize] = c as u32;
            freq[size] = freq[n as usize] + freq[d as usize];
            size += 1;
            c -= 1;
            freq[n as usize] = max as u32;
            freq[d as usize] = max as u32;
        }
        let root = size - 1;
        let input_size = input.len();
        Self {
            neighbour,
            root,
            input_size,
        }
    }
    fn read(&self, bit_stream: &mut BitStream) -> anyhow::Result<u32> {
        let mut ret = self.root as u32;
        while ret >= self.input_size as u32 {
            ret = self.neighbour[((bit_stream.read(1)? << 9) + ret) as usize];
        }
        Ok(ret)
    }
}

pub(crate) fn jbp1_decompress(buf: &[u8]) -> anyhow::Result<Vec<u8>> {
    let off = &mut 0;
    let jbp1 = Jbp1::new(buf)?;
    *off = jbp1.data_offset as usize;
    let mut freq_dc = vec![0; 128];
    for val in freq_dc.iter_mut().take(16) {
        *val = buf.gread_with::<u32>(off, LE)?;
    }
    let mut freq_ac = vec![0; 128];
    for val in freq_ac.iter_mut().take(16) {
        *val = buf.gread_with::<u32>(off, LE)?;
    }
    let tree_input = &mut buf
        .get(*off..*off + 16)
        .context("Out of bounds access")?
        .to_vec();
    *off += 16;
    tree_input.iter_mut().for_each(|b| *b += 1);

    let mut quant_y = vec![0i16; 128];
    let mut quant_c = vec![0i16; 128];
    if jbp1.flags & 0x08000000 != 0 {
        for val in quant_y.iter_mut().take(64) {
            *val = buf.gread::<u8>(off)? as i16;
        }
        for val in quant_c.iter_mut().take(64) {
            *val = buf.gread::<u8>(off)? as i16;
        }
    }

    let mut bit_stream_1 = BitStream::new(
        buf.get(*off..*off + jbp1.bit_pool_size_1 as usize)
            .context("Out of bounds access")?
            .to_vec(),
    );
    *off += jbp1.bit_pool_size_1 as usize;
    let mut bit_stream_2 = BitStream::new(
        buf.get(*off..*off + jbp1.bit_pool_size_2 as usize)
            .context("Out of bounce context")?
            .to_vec(),
    );
    *off += jbp1.bit_pool_size_2 as usize;
    let mut block_output = decode_blocks(
        &jbp1,
        tree_input,
        &mut bit_stream_1,
        &mut bit_stream_2,
        &mut freq_dc,
        &mut freq_ac,
        &mut quant_y,
        &mut quant_c,
    )?;

    if jbp1.depth != 32 {
        for p in block_output.chunks_mut(4) {
            p[3] = 0xFF;
        }
    }
    Ok(block_output)
}

#[allow(clippy::too_many_arguments)]
fn decode_blocks(
    jbp1: &Jbp1,
    tree_input: &[u8],
    bit_stream_1: &mut BitStream,
    bit_stream_2: &mut BitStream,
    freq_dc: &mut [u32],
    freq_ac: &mut [u32],
    quant_y: &mut [i16],
    quant_c: &mut [i16],
) -> anyhow::Result<Vec<u8>> {
    let tree_dc = Tree::new(tree_input, freq_dc);
    let tree_ac = Tree::new(tree_input, freq_ac);
    let mut blocks =
        vec![
            0;
            jbp1.x_block_count as usize * jbp1.y_block_count as usize * 3 * 2
        ];

    for i in 0..blocks.len() {
        let bit_count = tree_dc.read(bit_stream_1)?;
        let mut x = bit_stream_1.read(bit_count as usize)?;
        if x < (1 << (bit_count - 1)) {
            x = x - (1 << bit_count) + 1;
        }
        *blocks.get_mut(i).context("Out of bounds access")? = x;
        if i != 0 {
            *blocks.get_mut(i).context("Out of bounce access")? +=
                *blocks.get(i - 1).context("Out of bounds context")?;
        }
    }
    let mut block_output =
        vec![0; jbp1.blocks_width as usize * jbp1.blocks_height as usize * 4];
    let original_order = [
        1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33,
        40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50,
        43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46,
        53, 60, 61, 54, 47, 55, 62, 63, 0,
    ];

    for y in 0..jbp1.y_block_count as usize {
        let mut dst1 = y * jbp1.block_stride as usize * 16;
        let mut dst2 = dst1 + jbp1.block_stride as usize * 9;

        for x in 0..jbp1.x_block_count as usize {
            let mut dct_table: Vec<Vec<i16>> = vec![
                vec![0; 64],
                vec![0; 64],
                vec![0; 64],
                vec![0; 64],
                vec![0; 64],
                vec![0; 64],
            ];

            for n in 0..6 {
                *dct_table
                    .get_mut(n)
                    .context("Out of bounds access")?
                    .get_mut(0)
                    .context("Out of bounds access")? = *blocks
                    .get((y * jbp1.x_block_count as usize + x) * 6 + n)
                    .context("Out of bounds access")?
                    as i16;

                let mut i = 0;
                while i < 63 {
                    let bit_count = tree_ac.read(bit_stream_2)?;
                    if bit_count == 15 {
                        break;
                    }
                    if bit_count == 0 {
                        let mut tree_input_pos = 0;
                        while bit_stream_2.read(1)? != 0 {
                            tree_input_pos += 1;
                        }
                        i += tree_input
                            .get(tree_input_pos)
                            .context("Out of bounds access")?;
                    } else {
                        let mut x = bit_stream_2.read(bit_count as usize)?;
                        if x < (1 << (bit_count - 1)) {
                            x = x - (1 << bit_count) + 1;
                        }
                        *dct_table
                            .get_mut(n)
                            .context("Out of bounds access")?
                            .get_mut(
                                *original_order
                                    .get(i as usize)
                                    .context("Out of bounds access")?,
                            )
                            .context("Out of bounds access")? = x as i16;
                        i += 1;
                    }
                }
            }
            dct(&mut dct_table[0], quant_y);
            dct(&mut dct_table[1], quant_y);
            dct(&mut dct_table[2], quant_y);
            dct(&mut dct_table[3], quant_y);
            dct(&mut dct_table[4], quant_c);
            dct(&mut dct_table[5], quant_c);
            ycc2rgb(
                dst1,
                dst1 + jbp1.block_stride as usize,
                &dct_table[0],
                &dct_table[4],
                &dct_table[5],
                0,
                &mut block_output,
                jbp1.block_stride as usize,
            );
            ycc2rgb(
                dst1 + 32,
                dst1 + jbp1.block_stride as usize + 32,
                &dct_table[1],
                &dct_table[4],
                &dct_table[5],
                4,
                &mut block_output,
                jbp1.block_stride as usize,
            );
            ycc2rgb(
                dst2 - jbp1.block_stride as usize,
                dst2,
                &dct_table[2],
                &dct_table[4],
                &dct_table[5],
                32,
                &mut block_output,
                jbp1.block_stride as usize,
            );
            ycc2rgb(
                dst2 - jbp1.block_stride as usize + 32,
                dst2 + 32,
                &dct_table[3],
                &dct_table[4],
                &dct_table[5],
                36,
                &mut block_output,
                jbp1.block_stride as usize,
            );

            dst1 += 64;
            dst2 += 64;
        }
    }
    Ok(block_output)
}

#[allow(clippy::many_single_char_names)]
fn dct(dct_table: &mut [i16], quant: &mut [i16]) {
    let mut lp1 = &mut dct_table[..];
    let mut lp2 = &mut quant[..];

    let mut a: isize;
    let mut b: isize;
    let mut c: isize;
    let mut d: isize;
    let mut w: isize;
    let mut x: isize;
    let mut y: isize;
    let mut z: isize;
    let mut s: isize;
    let mut t: isize;
    let mut u: isize;
    let mut v: isize;
    let mut n: isize;

    for _ in 0..8 {
        if lp1[0x08] == 0
            && lp1[0x10] == 0
            && lp1[0x18] == 0
            && lp1[0x20] == 0
            && lp1[0x28] == 0
            && lp1[0x30] == 0
            && lp1[0x38] == 0
        {
            let val = lp1[0] * lp2[0];
            lp1[0x00] = val;
            lp1[0x08] = val;
            lp1[0x10] = val;
            lp1[0x18] = val;
            lp1[0x20] = val;
            lp1[0x28] = val;
            lp1[0x30] = val;
            lp1[0x38] = val;
        } else {
            c = (lp2[0x10] * lp1[0x10]) as isize;
            d = (lp2[0x30] * lp1[0x30]) as isize;
            x = ((c + d) * 35467) >> 16;
            c = ((c * 50159) >> 16) + x;
            d = ((d * -121094) >> 16) + x;
            a = (lp1[0x00] * lp2[0x00]) as isize;
            b = (lp1[0x20] * lp2[0x20]) as isize;
            w = a + b + c;
            x = a + b - c;
            y = a - b + d;
            z = a - b - d;

            c = (lp1[0x38] * lp2[0x38]) as isize;
            d = (lp1[0x28] * lp2[0x28]) as isize;
            a = (lp1[0x18] * lp2[0x18]) as isize;
            b = (lp1[0x08] * lp2[0x08]) as isize;
            n = ((a + b + c + d) * 77062) >> 16;

            u = n
                + ((c * 19571) >> 16)
                + (((c + a) * -128553) >> 16)
                + (((c + b) * -58980) >> 16);
            v = n
                + ((d * 134553) >> 16)
                + (((d + b) * -25570) >> 16)
                + (((d + a) * -167963) >> 16);
            t = n
                + ((b * 98390) >> 16)
                + (((d + b) * -25570) >> 16)
                + (((c + b) * -58980) >> 16);
            s = n
                + ((a * 201373) >> 16)
                + (((c + a) * -128553) >> 16)
                + (((d + a) * -167963) >> 16);

            lp1[0x00] = (w + t) as i16;
            lp1[0x38] = (w - t) as i16;
            lp1[0x08] = (y + s) as i16;
            lp1[0x30] = (y - s) as i16;
            lp1[0x10] = (z + v) as i16;
            lp1[0x28] = (z - v) as i16;
            lp1[0x18] = (x + u) as i16;
            lp1[0x20] = (x - u) as i16;
        }
        lp1 = &mut lp1[1..];
        lp2 = &mut lp2[1..];
    }

    lp1 = &mut dct_table[..];

    for _ in 0..8 {
        a = lp1[0] as isize;
        c = lp1[2] as isize;
        b = lp1[4] as isize;
        d = lp1[6] as isize;
        x = ((c + d) * 35467) >> 16;
        c = ((c * 50159) >> 16) + x;
        d = ((d * -121094) >> 16) + x;
        w = a + b + c;
        x = a + b - c;
        y = a - b + d;
        z = a - b - d;

        d = lp1[5] as isize;
        b = lp1[1] as isize;
        c = lp1[7] as isize;
        a = lp1[3] as isize;
        n = ((a + b + c + d) * 77062) >> 16;

        s = n
            + ((a * 201373) >> 16)
            + (((a + c) * -128553) >> 16)
            + (((a + d) * -167963) >> 16);

        t = n
            + ((b * 98390) >> 16)
            + (((b + d) * -25570) >> 16)
            + (((b + c) * -58980) >> 16);

        u = n
            + ((c * 19571) >> 16)
            + (((b + c) * -58980) >> 16)
            + (((a + c) * -128553) >> 16);

        v = n
            + ((d * 134553) >> 16)
            + (((b + d) * -25570) >> 16)
            + (((a + d) * -167963) >> 16);

        lp1[0] = ((w + t) >> 3) as i16;
        lp1[7] = ((w - t) >> 3) as i16;
        lp1[1] = ((y + s) >> 3) as i16;
        lp1[6] = ((y - s) >> 3) as i16;
        lp1[2] = ((z + v) >> 3) as i16;
        lp1[5] = ((z - v) >> 3) as i16;
        lp1[3] = ((x + u) >> 3) as i16;
        lp1[4] = ((x - u) >> 3) as i16;

        lp1 = &mut lp1[8..];
    }
}

#[allow(clippy::too_many_arguments)]
fn ycc2rgb(
    mut dc: usize,
    mut ac: usize,
    dct_y: &[i16],
    dct_cb: &[i16],
    dct_cr: &[i16],
    mut cbcr_src: usize,
    m_output: &mut [u8],
    m_stride: usize,
) {
    let mut y_src = 0;
    for _ in 0..4 {
        for _ in 0..4 {
            let cb = dct_cb[cbcr_src] as isize;
            let cr = dct_cr[cbcr_src] as isize;
            let r = ((cr * 0x166F0) >> 16) as usize;
            let g = (((cb * 0x5810) >> 16) + ((cr * 0xB6C0) >> 16)) as usize;
            let b = ((cb * 0x1C590) >> 16) as usize;
            let c0 = dct_y[y_src] as usize + 0x180;
            let c1 = dct_y[y_src + 1] as usize + 0x180;
            let c8 = dct_y[y_src + 8] as usize + 0x180;
            let c9 = dct_y[y_src + 9] as usize + 0x180;
            m_output[dc] = LOOKUP_TABLE[c0 + b];
            m_output[ac + 1 - m_stride] = LOOKUP_TABLE[c0 - g];
            m_output[ac + 2 - m_stride] = LOOKUP_TABLE[c0 + r];
            m_output[ac + 4 - m_stride] = LOOKUP_TABLE[c1 + b];
            m_output[ac + 5 - m_stride] = LOOKUP_TABLE[c1 - g];
            m_output[ac + 6 - m_stride] = LOOKUP_TABLE[c1 + r];
            m_output[ac] = LOOKUP_TABLE[c8 + b];
            m_output[ac + 1] = LOOKUP_TABLE[c8 - g];
            m_output[ac + 2] = LOOKUP_TABLE[c8 + r];
            m_output[ac + 4] = LOOKUP_TABLE[c9 + b];
            m_output[ac + 5] = LOOKUP_TABLE[c9 - g];
            m_output[ac + 6] = LOOKUP_TABLE[c9 + r];
            y_src += 2;
            dc += 8;
            ac += 8;
            cbcr_src += 1;
        }
        dc += m_stride * 2 - 32;
        ac += m_stride * 2 - 32;

        y_src += 8;
        cbcr_src += 4;
    }
}
