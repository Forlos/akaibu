use scroll::{Pread, LE};

pub(crate) fn psubb(p1: &mut [u8; 4], p2: &[u8; 4], bytes_per_pixel: usize) {
    for i in 0..bytes_per_pixel {
        p1[i] = p1[i].wrapping_sub(p2[i]);
    }
}

pub(crate) fn punpcklbw0(xmm0: [u8; 4]) -> [u8; 8] {
    let mut dest = [0; 8];
    for i in 0..4 {
        dest[i * 2] = xmm0[i];
    }
    dest
}

pub(crate) fn paddw(mm0: &mut [u8; 8], mm1: &[u8; 8]) -> anyhow::Result<()> {
    for i in 0..4 {
        let v = mm0[i * 2..i * 2 + 2]
            .pread_with::<u16>(0, LE)?
            .wrapping_add(mm1[i * 2..i * 2 + 2].pread_with::<u16>(0, LE)?);
        mm0[i * 2..i * 2 + 2].copy_from_slice(&v.to_le_bytes());
    }
    Ok(())
}

pub(crate) fn psrlw(mm0: &mut [u8; 8], x: u32) -> anyhow::Result<()> {
    mm0.chunks_exact_mut(2)
        .try_for_each::<_, anyhow::Result<()>>(|c| {
            let mut v = c.pread_with::<u16>(0, LE)?;
            v = v.wrapping_shr(x);
            c.copy_from_slice(&v.to_le_bytes());
            Ok(())
        })
}

pub(crate) fn packuswb0(xmm0: [u8; 8]) -> anyhow::Result<[u8; 4]> {
    let mut result = [0; 4];
    for i in 0..4 {
        let b = xmm0.pread_with::<i16>(i * 2, LE)?;
        result[i] = if b > 0xFF {
            0xFF
        } else if b < 0 {
            0
        } else {
            b as u8
        };
    }
    Ok(result)
}
