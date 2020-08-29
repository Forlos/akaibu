use encoding_rs::SHIFT_JIS;
use scroll::{ctx, IOread, Pread, LE};

use super::Scheme;
use crate::{
    archive,
    util::{crc64, zlib_decompress},
};
use anyhow::Context;
use bytes::{Bytes, BytesMut};
use std::io::{Read, Seek};
use std::{collections::HashMap, fs::File, path::PathBuf};

const MASTER_KEY: u32 = 0x8B6A4E5F;

#[derive(Debug)]
pub enum Acv1Scheme {
    Shukugar1,
    Shukugar2,
    Shukugar3,
    HanaHime,
}

impl Scheme for Acv1Scheme {
    fn extract(
        &self,
        file_path: &PathBuf,
    ) -> anyhow::Result<Box<dyn archive::Archive + Sync>> {
        let file_names = crate::Resources::get("acv1/all_file_names.txt")
            .context("Could not get resouce")?;
        let (sjis_file_names, _encoding_used, _any_errors) =
            SHIFT_JIS.decode(&file_names);

        let mut hashes = HashMap::new();
        sjis_file_names.lines().for_each(|l| {
            hashes.insert(crc64(&SHIFT_JIS.encode(&l).0), l);
        });
        let mut file = File::open(file_path)?;
        file.seek(std::io::SeekFrom::Start(4))?;
        let entries_count = file.ioread_with::<u32>(LE)? ^ MASTER_KEY;
        let mut buf = vec![0; 4 + entries_count as usize * 21];
        file.read_exact(&mut buf)?;

        let acv = buf.pread_with::<Acv1>(0, (entries_count, &hashes))?;
        log::debug!("Archive: {:?}", acv);

        Ok(Box::new(Acv1Archive {
            file_path: file_path.clone(),
            archive: acv,
            script_key: self.get_script_key(),
        }))
    }
    fn get_name(&self) -> &str {
        match self {
            Self::Shukugar1 => {
                "Shukusei no Girlfriend -the destiny star of girlfriend-"
            }
            Self::Shukugar2 => {
                "Shukusei no Girlfriend 2 -the destiny star of girlfriend-"
            }
            Self::Shukugar3 => {
                "Shukusei no Girlfriend 3 -the destiny star of girlfriend-"
            }
            Self::HanaHime => "Hana Hime * Absolute!",
        }
    }
    fn get_schemes() -> Vec<Box<dyn Scheme>>
    where
        Self: Sized,
    {
        vec![
            Box::new(Acv1Scheme::Shukugar1),
            Box::new(Acv1Scheme::Shukugar2),
            Box::new(Acv1Scheme::Shukugar3),
            Box::new(Acv1Scheme::HanaHime),
        ]
    }
}

impl Acv1Scheme {
    fn get_script_key(&self) -> u32 {
        match self {
            Self::Shukugar1 => 0x9d0be0fa,
            Self::Shukugar2 => 0xcf762ea8,
            Self::Shukugar3 => 0x3548751d,
            Self::HanaHime => 0x30bc61c8,
        }
    }
}

#[derive(Debug)]
struct Acv1Archive {
    file_path: PathBuf,
    script_key: u32,
    archive: Acv1,
}

impl archive::Archive for Acv1Archive {
    fn get_files(&self) -> Vec<archive::FileEntry> {
        self.archive
            .file_entries
            .iter()
            .filter(|e| e.extractable)
            .map(|e| archive::FileEntry {
                file_name: e.file_name.clone(),
                file_offset: e.file_offset as usize,
                file_size: e.file_size as usize,
            })
            .collect()
    }
    fn extract(
        &self,
        entry: &archive::FileEntry,
    ) -> anyhow::Result<bytes::Bytes> {
        self.archive
            .file_entries
            .iter()
            .filter(|e| e.extractable)
            .find(|e| e.file_name == entry.file_name)
            .map(|e| self.extract(e))
            .context("File not found")?
    }
}

impl Acv1Archive {
    fn extract(&self, entry: &Acv1Entry) -> anyhow::Result<Bytes> {
        if entry.flags == 6 {
            log::debug!("Extracting script: {:X?}", entry);
            Ok(entry.dump_script(&self.file_path, self.script_key)?)
        } else {
            log::debug!("Extracting resource: {:X?}", entry);
            Ok(entry.dump_entry(&self.file_path)?)
        }
    }
}

#[derive(Debug)]
struct Acv1 {
    file_entries: Vec<Acv1Entry>,
}

impl<'a> ctx::TryFromCtx<'a, (u32, &HashMap<u64, &str>)> for Acv1 {
    type Error = anyhow::Error;
    #[inline]
    fn try_from_ctx(
        buf: &'a [u8],
        (entry_count, hashes): (u32, &HashMap<u64, &str>),
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let mut file_entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            file_entries.push(buf.gread_with(off, hashes)?)
        }
        Ok((Acv1 { file_entries }, 4))
    }
}

#[derive(Debug)]
struct Acv1Entry {
    crc64: u64,
    flags: u8,
    file_offset: u32,
    file_size: u32,
    uncompressed_file_size: u32,
    file_name: String,
    /// File is not extractable when:
    /// - its is not a script file
    /// - there is no file name for its crc64 in acv1/all_file_names.txt file
    extractable: bool,
}

impl<'a> ctx::TryFromCtx<'a, &HashMap<u64, &str>> for Acv1Entry {
    type Error = anyhow::Error;
    #[inline]
    fn try_from_ctx(
        buf: &'a [u8],
        hashes: &HashMap<u64, &str>,
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let crc64 = buf.gread_with::<u64>(off, LE)?;
        let xor_key = crc64 as u32;

        let flags = buf.gread_with::<u8>(off, LE)? ^ xor_key as u8;
        let mut file_offset =
            buf.gread_with::<u32>(off, LE)? ^ xor_key ^ MASTER_KEY;
        let mut file_size = buf.gread_with::<u32>(off, LE)? ^ xor_key;
        let mut uncompressed_file_size =
            buf.gread_with::<u32>(off, LE)? ^ xor_key;
        let mut extractable = true;

        let file_name = if let Some(v) = hashes.get(&crc64) {
            let file_name = v.to_string();
            let name = SHIFT_JIS.encode(&file_name).0;
            if flags & 2 == 0 {
                file_offset ^= name[name.len() >> 1] as u32;
                file_size ^= name[name.len() >> 2] as u32;
                uncompressed_file_size ^= name[name.len() >> 3] as u32;
            }
            file_name
        } else if flags & 4 >= 1 {
            format!("{:X}", crc64)
        } else {
            extractable = false;
            "".to_string()
        };
        Ok((
            Acv1Entry {
                crc64,
                flags,
                file_offset,
                file_size,
                uncompressed_file_size,
                file_name,
                extractable,
            },
            21,
        ))
    }
}

impl Acv1Entry {
    fn dump_entry(&self, file_path: &PathBuf) -> anyhow::Result<Bytes> {
        let mut buf = BytesMut::new();
        buf.resize(self.file_size as usize, 0);
        // let mut buf = vec![0; self.file_size as usize];
        let mut file = File::open(&file_path)?;
        file.seek(std::io::SeekFrom::Start(self.file_offset as u64))?;
        file.read_exact(&mut buf)?;

        if self.flags == 0 {
            return Ok(buf.freeze());
        }
        if self.flags & 2 == 0 {
            let name = SHIFT_JIS.encode(&self.file_name).0;
            let result = self.file_size as usize / name.len();
            let mut index = 0_usize;
            let mut name_index = 0_usize;
            while index <= self.file_size as usize
                && name_index < name.len() - 1
            {
                for _ in 0..result {
                    buf[index] ^= name[name_index];
                    index += 1;
                }
                name_index += 1;
            }
            return Ok(buf.freeze());
        }
        let xor_key = self.crc64 as u32;
        buf.chunks_exact_mut(4).for_each(|c| {
            c[0] ^= xor_key as u8;
            c[1] ^= (xor_key >> 8) as u8;
            c[2] ^= (xor_key >> 16) as u8;
            c[3] ^= (xor_key >> 24) as u8;
        });
        zlib_decompress(&buf)
    }
    fn dump_script(
        &self,
        file_path: &PathBuf,
        script_key: u32,
    ) -> anyhow::Result<Bytes> {
        let mut buf = BytesMut::new();
        buf.resize(self.file_size as usize, 0);
        // let mut buf = vec![0; self.file_size as usize];
        let mut file = File::open(&file_path)?;
        file.seek(std::io::SeekFrom::Start(self.file_offset as u64))?;
        file.read_exact(&mut buf)?;

        let xor_key = self.crc64 as u32 ^ script_key;
        buf.chunks_exact_mut(4).for_each(|c| {
            c[0] ^= xor_key as u8;
            c[1] ^= (xor_key >> 8) as u8;
            c[2] ^= (xor_key >> 16) as u8;
            c[3] ^= (xor_key >> 24) as u8;
        });

        zlib_decompress(&buf)
    }
}
