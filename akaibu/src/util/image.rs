pub fn bitmap_to_png(buf: Vec<u8>, width_in_bytes: usize) -> Vec<u8> {
    buf.chunks_exact(width_in_bytes)
        .rev()
        .flatten()
        .copied()
        .collect()
}

pub fn bitmap_to_png_with_padding(
    buf: Vec<u8>,
    width_in_bytes: usize,
    padding: usize,
) -> Vec<u8> {
    if padding == 0 {
        bitmap_to_png(buf, width_in_bytes)
    } else {
        buf.chunks_exact(width_in_bytes)
            .map(|c| &c[..width_in_bytes - padding])
            .rev()
            .flatten()
            .copied()
            .collect()
    }
}
