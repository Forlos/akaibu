use super::Scheme;
use crate::archive::{self, FileContents};
use anyhow::Context;
use bytes::BytesMut;
use encoding_rs::SHIFT_JIS;
use once_cell::sync::Lazy;
use positioned_io::{RandomAccessFile, ReadAt};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use scroll::{Pread, LE};
use std::{collections::HashMap, fs::File, io::Write, path::PathBuf};

#[derive(Debug, Clone)]
pub enum ArcScheme {
    // TODO: this one has different scheme
    // Maou1,
    Maou2,
    Maou2FD,
    Oshioki,
}

const KEYS_PATH: &str = "tactics_arc/keys.json";

static KEYS: Lazy<HashMap<String, String>> = Lazy::new(|| {
    let keys = serde_json::from_slice(
        &crate::Resources::get(KEYS_PATH)
            .expect("Could not find file: tactics_arc/keys.json"),
    )
    .expect("Could not deserialize resource json");
    keys
});

impl Scheme for ArcScheme {
    fn extract(
        &self,
        file_path: &std::path::Path,
    ) -> anyhow::Result<(
        Box<dyn crate::archive::Archive>,
        crate::archive::NavigableDirectory,
    )> {
        let metadata = std::fs::metadata(&file_path)?;
        let mut buf = vec![0; 20];
        let file = RandomAccessFile::open(file_path)?;
        let mut cur_file_offset = 16;

        let mut file_entries = Vec::new();

        while cur_file_offset < metadata.len() {
            file.read_exact_at(cur_file_offset, &mut buf)?;

            let file_size = buf.pread_with::<u32>(0, LE)? as u64;
            let decompressed_file_size = buf.pread_with::<u32>(4, LE)? as usize;
            let name_size = buf.pread_with::<u32>(8, LE)? as usize;

            let mut file_name_buf = vec![0; name_size];
            cur_file_offset += 20;
            file.read_exact_at(cur_file_offset, &mut file_name_buf)?;

            cur_file_offset += name_size as u64;

            if name_size > 0 {
                file_entries.push(ArcFileEntry {
                    file_size,
                    decompressed_file_size,
                    file_offset: cur_file_offset,
                    full_path: PathBuf::from(
                        SHIFT_JIS.decode(&file_name_buf).0.replace("\\", "/"),
                    ),
                });
            }

            cur_file_offset += file_size as u64
        }
        let root_dir = ArcArchive::new_root_dir(&file_entries);
        let navigable_dir = archive::NavigableDirectory::new(root_dir);
        let xor_key = KEYS
            .get(match self {
                // ArcScheme::Maou1 => "Maou1",
                ArcScheme::Maou2 => "Maou2",
                ArcScheme::Maou2FD => "Maou2FD",
                ArcScheme::Oshioki => "Oshioki",
            })
            .context(format!("Could not find key for {:?}", self))?
            .clone()
            .into_bytes();
        Ok((
            Box::new(ArcArchive {
                file,
                file_entries,
                xor_key,
            }),
            navigable_dir,
        ))
    }

    fn get_name(&self) -> String {
        format!(
            "[TACTICS_ARC_FILE] {}",
            match self {
                // Self::Maou1 => "Maou no Kuse ni Namaiki da!",
                Self::Maou2 =>
                    "Maou no Kuse ni Namaiki da! 2 ~Kondo wa Seisen da!~",
                Self::Maou2FD =>
                    "Maou no Kuse ni Namaiki da! Torotoro Tropical!",
                Self::Oshioki =>
                    "Akuma de Oshioki! Marukido Sadoshiki Hentai Oshioki Kouza",
            }
        )
    }

    fn get_schemes() -> Vec<Box<dyn Scheme>>
    where
        Self: Sized,
    {
        vec![
            // Box::new(Self::Maou1),
            Box::new(Self::Maou2),
            Box::new(Self::Maou2FD),
            Box::new(Self::Oshioki),
        ]
    }
}

#[derive(Debug)]
struct ArcArchive {
    file: RandomAccessFile,
    file_entries: Vec<ArcFileEntry>,
    xor_key: Vec<u8>,
}

impl archive::Archive for ArcArchive {
    fn extract(
        &self,
        entry: &archive::FileEntry,
    ) -> anyhow::Result<archive::FileContents> {
        self.file_entries
            .iter()
            .find(|e| e.full_path == entry.full_path)
            .map(|e| self.extract(e))
            .context("File not found")?
    }

    fn extract_all(&self, output_path: &std::path::Path) -> anyhow::Result<()> {
        self.file_entries.par_iter().try_for_each(|entry| {
            let file_contents = self.extract(entry)?;
            let mut output_file_name = PathBuf::from(output_path);
            output_file_name.push(&entry.full_path);
            std::fs::create_dir_all(
                &output_file_name
                    .parent()
                    .context("Could not get parent directory")?,
            )?;
            log::debug!(
                "Extracting resource: {:?} {:X?}",
                output_file_name,
                entry
            );
            File::create(output_file_name)?
                .write_all(&file_contents.contents)?;
            Ok(())
        })
    }
}

impl ArcArchive {
    fn new_root_dir(entries: &[ArcFileEntry]) -> archive::Directory {
        archive::Directory::new(
            entries
                .iter()
                .map(|entry| {
                    let file_offset = entry.file_offset;
                    let file_size = entry.file_size;
                    archive::FileEntry {
                        file_name: entry
                            .full_path
                            .to_str()
                            .expect("Not valid UTF-8")
                            .to_string(),
                        full_path: entry.full_path.clone(),
                        file_offset,
                        file_size,
                    }
                })
                .collect(),
        )
    }
    fn extract(&self, entry: &ArcFileEntry) -> anyhow::Result<FileContents> {
        let mut buf = BytesMut::with_capacity(entry.file_size as usize);
        buf.resize(entry.file_size as usize, 0);

        self.file
            .read_exact_at(entry.file_offset as u64, &mut buf)?;
        buf.iter_mut()
            .zip(self.xor_key.iter().cycle())
            .for_each(|(b, k)| *b ^= k);
        Ok(FileContents {
            // contents: bytes::Bytes::copy_from_slice(&buf[4..]),
            contents: bytes::Bytes::from(decompress(&buf)),
            type_hint: None,
        })
    }
}

#[derive(Debug)]
struct ArcFileEntry {
    file_size: u64,
    decompressed_file_size: usize,
    file_offset: u64,
    full_path: PathBuf,
}

/* fn decompress(src: &[u8], dest_len: usize) -> Vec<u8> {
    let mut dest_index = 0;
    let mut src_index = 0;
    let mut buf_index = 0xfee;
    let mut buf = [0u8; 4096];
    let mut flag = 0u16;

    let mut dest = vec![0; dest_len];
    loop {
        flag >>= 1;
        if (flag & 0x100) == 0 {
            flag = src[src_index] as u16 | 0xFF00;
            src_index += 1;
        }
        if (flag & 1) != 0 {
            let d = src[src_index];
            src_index += 1;
            dest[dest_index] = d;
            dest_index += 1;
            if dest_index == dest_len {
                return dest;
            }
            buf[buf_index] = d;
            buf_index += 1;
            buf_index &= buf.len() - 1;
        } else {
            let mut temp_buf_index = src[src_index] as usize;
            src_index += 1;
            let mut counter = src[src_index] as usize;
            src_index += 1;
            temp_buf_index |= (counter >> 4) << 8;
            counter &= 0xF;
            counter += 3;

            for i in 0..counter {
                let d = buf[(temp_buf_index + i) & (buf.len() - 1)];
                dest[dest_index] = d;
                dest_index += 1;
                if dest_index == dest_len {
                    return dest;
                }
                buf[buf_index] = d;
                buf_index += 1;
                buf_index &= buf.len() - 1;
            }
        }
    }
} */
const DECOMPRESS_TABLE: &[u16] = &[
    0x0001, 0x0804, 0x1001, 0x2001, 0x0002, 0x0805, 0x1002, 0x2002, 0x0003,
    0x0806, 0x1003, 0x2003, 0x0004, 0x0807, 0x1004, 0x2004, 0x0005, 0x0808,
    0x1005, 0x2005, 0x0006, 0x0809, 0x1006, 0x2006, 0x0007, 0x080A, 0x1007,
    0x2007, 0x0008, 0x080B, 0x1008, 0x2008, 0x0009, 0x0904, 0x1009, 0x2009,
    0x000A, 0x0905, 0x100A, 0x200A, 0x000B, 0x0906, 0x100B, 0x200B, 0x000C,
    0x0907, 0x100C, 0x200C, 0x000D, 0x0908, 0x100D, 0x200D, 0x000E, 0x0909,
    0x100E, 0x200E, 0x000F, 0x090A, 0x100F, 0x200F, 0x0010, 0x090B, 0x1010,
    0x2010, 0x0011, 0x0A04, 0x1011, 0x2011, 0x0012, 0x0A05, 0x1012, 0x2012,
    0x0013, 0x0A06, 0x1013, 0x2013, 0x0014, 0x0A07, 0x1014, 0x2014, 0x0015,
    0x0A08, 0x1015, 0x2015, 0x0016, 0x0A09, 0x1016, 0x2016, 0x0017, 0x0A0A,
    0x1017, 0x2017, 0x0018, 0x0A0B, 0x1018, 0x2018, 0x0019, 0x0B04, 0x1019,
    0x2019, 0x001A, 0x0B05, 0x101A, 0x201A, 0x001B, 0x0B06, 0x101B, 0x201B,
    0x001C, 0x0B07, 0x101C, 0x201C, 0x001D, 0x0B08, 0x101D, 0x201D, 0x001E,
    0x0B09, 0x101E, 0x201E, 0x001F, 0x0B0A, 0x101F, 0x201F, 0x0020, 0x0B0B,
    0x1020, 0x2020, 0x0021, 0x0C04, 0x1021, 0x2021, 0x0022, 0x0C05, 0x1022,
    0x2022, 0x0023, 0x0C06, 0x1023, 0x2023, 0x0024, 0x0C07, 0x1024, 0x2024,
    0x0025, 0x0C08, 0x1025, 0x2025, 0x0026, 0x0C09, 0x1026, 0x2026, 0x0027,
    0x0C0A, 0x1027, 0x2027, 0x0028, 0x0C0B, 0x1028, 0x2028, 0x0029, 0x0D04,
    0x1029, 0x2029, 0x002A, 0x0D05, 0x102A, 0x202A, 0x002B, 0x0D06, 0x102B,
    0x202B, 0x002C, 0x0D07, 0x102C, 0x202C, 0x002D, 0x0D08, 0x102D, 0x202D,
    0x002E, 0x0D09, 0x102E, 0x202E, 0x002F, 0x0D0A, 0x102F, 0x202F, 0x0030,
    0x0D0B, 0x1030, 0x2030, 0x0031, 0x0E04, 0x1031, 0x2031, 0x0032, 0x0E05,
    0x1032, 0x2032, 0x0033, 0x0E06, 0x1033, 0x2033, 0x0034, 0x0E07, 0x1034,
    0x2034, 0x0035, 0x0E08, 0x1035, 0x2035, 0x0036, 0x0E09, 0x1036, 0x2036,
    0x0037, 0x0E0A, 0x1037, 0x2037, 0x0038, 0x0E0B, 0x1038, 0x2038, 0x0039,
    0x0F04, 0x1039, 0x2039, 0x003A, 0x0F05, 0x103A, 0x203A, 0x003B, 0x0F06,
    0x103B, 0x203B, 0x003C, 0x0F07, 0x103C, 0x203C, 0x0801, 0x0F08, 0x103D,
    0x203D, 0x1001, 0x0F09, 0x103E, 0x203E, 0x1801, 0x0F0A, 0x103F, 0x203F,
    0x2001, 0x0F0B, 0x1040, 0x2040,
];

fn decompress(src: &[u8]) -> Vec<u8> {
    let mut decompressed_size = 0;
    let mut src_index = 0;
    let mut dest_index = 0;
    let mut b = 0xFF;

    let mut i = 0;
    while b >= 0x80 {
        b = src[src_index];
        src_index += 1;
        decompressed_size |= ((b as u32 & 0x7F) << i) as usize;
        i += 7;
    }

    let mut dest = vec![0u8; decompressed_size];

    while dest_index < decompressed_size {
        b = src[src_index];
        src_index += 1;
        if (b & 3) != 0 {
            let offset_length =
                (DECOMPRESS_TABLE[b as usize] as u32 >> 8) & 0xFFFF_FFF8;
            let mut offset = 0u32;
            let mut i = 0;
            while i < offset_length {
                offset |= (src[src_index] as u32) << i;
                src_index += 1;
                i += 8;
            }
            offset = offset
                .wrapping_add((DECOMPRESS_TABLE[b as usize] & 0x700) as u32);

            let offset = offset as usize;
            let count = (DECOMPRESS_TABLE[b as usize] as u8) as usize;
            dest.copy_within(
                dest_index - offset..dest_index - offset + count,
                dest_index,
            );
            dest_index += count as usize;
        } else {
            let mut count = (b as u32 >> 2) + 1;
            if count >= 0x3D {
                let count_length = (count - 0x3C) * 8;
                count = 0;
                let mut i = 0;
                while i < count_length {
                    count |= (src[src_index] as u32) << i;
                    src_index += 1;
                    i += 8;
                }
                count += 1;
            }
            dest[dest_index..dest_index + count as usize]
                .copy_from_slice(&src[src_index..src_index + count as usize]);
            src_index += count as usize;
            dest_index += count as usize;
        }
    }
    dest
}
