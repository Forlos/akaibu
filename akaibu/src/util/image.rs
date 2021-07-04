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

pub fn remove_bitmap_padding(
    buf: Vec<u8>,
    width_in_bytes: usize,
    padding: usize,
) -> Vec<u8> {
    buf.chunks_exact(width_in_bytes)
        .map(|c| &c[..width_in_bytes - padding])
        .flatten()
        .copied()
        .collect()
}

pub fn resolve_color_table(
    color_index_table: &[u8],
    color_table: &[u8],
) -> Vec<u8> {
    color_index_table.iter().fold(
        Vec::with_capacity(color_index_table.len() * 4),
        |mut v, b| {
            v.extend_from_slice(
                &color_table[*b as usize * 4..*b as usize * 4 + 4],
            );
            v
        },
    )
}

pub fn resolve_color_table_without_alpha(
    color_index_table: &[u8],
    color_table: &[u8],
) -> Vec<u8> {
    color_index_table.iter().fold(
        Vec::with_capacity(color_index_table.len() * 3),
        |mut v, b| {
            v.extend_from_slice(
                &color_table[*b as usize * 3..*b as usize * 3 + 3],
            );
            v
        },
    )
}
