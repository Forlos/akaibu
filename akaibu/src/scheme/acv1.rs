use super::Scheme;
use crate::{
    archive::{self, FileContents},
    util::{crc64, zlib_decompress},
};
use anyhow::Context;
use bytes::{Bytes, BytesMut};
use encoding_rs::SHIFT_JIS;
use positioned_io::{RandomAccessFile, ReadAt};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use scroll::{ctx, Pread, LE};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

const MASTER_KEY: u32 = 0x8B6A4E5F;

#[derive(Debug, Clone)]
pub enum Acv1Scheme {
    Shukugar1,
    Shukugar2,
    Shukugar3,
    HanaHime,
}

impl Scheme for Acv1Scheme {
    fn extract(
        &self,
        file_path: &Path,
    ) -> anyhow::Result<(Box<dyn archive::Archive>, archive::NavigableDirectory)>
    {
        let file_names = crate::Resources::get("acv1/all_file_names.txt")
            .context("Could not get resouce")?;
        let (sjis_file_names, _encoding_used, _any_errors) =
            SHIFT_JIS.decode(&file_names);

        let mut hashes = BTreeMap::new();
        sjis_file_names.lines().for_each(|l| {
            hashes.insert(crc64(&SHIFT_JIS.encode(&l).0), l);
        });
        let mut buf = vec![0; 4];
        let file = RandomAccessFile::open(file_path)?;
        file.read_exact_at(4, &mut buf)?;
        let entries_count = buf.pread_with::<u32>(0, LE)? ^ MASTER_KEY;
        let mut buf = vec![0; 4 + entries_count as usize * 21];
        file.read_exact_at(8, &mut buf)?;

        let archive = buf.pread_with::<Acv1>(0, (entries_count, &hashes))?;
        log::debug!("Archive: {:?}", archive);

        let root_dir = Acv1Archive::new_root_dir(&archive.file_entries);
        let navigable_dir = archive::NavigableDirectory::new(root_dir);
        Ok((
            Box::new(Acv1Archive {
                file,
                archive,
                script_key: self.get_script_key(),
            }),
            navigable_dir,
        ))
    }
    fn get_name(&self) -> String {
        format!(
            "[ACV1] {}",
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
        )
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
    file: RandomAccessFile,
    script_key: u32,
    archive: Acv1,
}

impl archive::Archive for Acv1Archive {
    fn extract(
        &self,
        entry: &archive::FileEntry,
    ) -> anyhow::Result<FileContents> {
        self.archive
            .file_entries
            .iter()
            .filter(|e| e.extractable)
            .find(|e| e.full_path == entry.full_path)
            .map(|e| self.extract(e))
            .context("File not found")?
    }

    fn extract_all(&self, output_path: &Path) -> anyhow::Result<()> {
        self.archive.file_entries.par_iter().try_for_each(|entry| {
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

impl Acv1Archive {
    fn new_root_dir(entries: &[Acv1Entry]) -> archive::Directory {
        archive::Directory::new(
            entries
                .iter()
                .filter(|entry| entry.extractable)
                .map(|entry| {
                    let file_offset = entry.file_offset as u64;
                    let file_size = entry.file_size as u64;
                    archive::FileEntry {
                        file_name: String::from(
                            entry
                                .full_path
                                .file_name()
                                .expect("No file name")
                                .to_str()
                                .expect("Not valid UTF-8"),
                        ),
                        full_path: entry.full_path.clone(),
                        file_offset,
                        file_size,
                    }
                })
                .collect(),
        )
    }
    fn extract(&self, entry: &Acv1Entry) -> anyhow::Result<FileContents> {
        if entry.flags == 6 {
            log::debug!("Extracting script: {:X?}", entry);
            Ok(FileContents {
                contents: entry.dump_script(&self.file, self.script_key)?,
                type_hint: None,
            })
        } else {
            log::debug!("Extracting resource: {:X?}", entry);
            Ok(FileContents {
                contents: entry.dump_entry(&self.file)?,
                type_hint: None,
            })
        }
    }
}

#[derive(Debug)]
struct Acv1 {
    file_entries: Vec<Acv1Entry>,
}

impl<'a> ctx::TryFromCtx<'a, (u32, &BTreeMap<u64, &str>)> for Acv1 {
    type Error = anyhow::Error;
    #[inline]
    fn try_from_ctx(
        buf: &'a [u8],
        (entry_count, hashes): (u32, &BTreeMap<u64, &str>),
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
    full_path: PathBuf,
    /// File is not extractable when:
    /// - its is not a script file
    /// - there is no file name for its crc64 in acv1/all_file_names.txt file
    extractable: bool,
}

impl<'a> ctx::TryFromCtx<'a, &BTreeMap<u64, &str>> for Acv1Entry {
    type Error = anyhow::Error;
    #[inline]
    fn try_from_ctx(
        buf: &'a [u8],
        hashes: &BTreeMap<u64, &str>,
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

        let full_path = PathBuf::from(if let Some(v) = hashes.get(&crc64) {
            let file_name = v.to_string();
            let name = SHIFT_JIS.encode(&file_name).0;
            if flags & 2 == 0 {
                file_offset ^= *name
                    .get(name.len() >> 1)
                    .context("Out of bounds access")?
                    as u32;
                file_size ^= *name
                    .get(name.len() >> 2)
                    .context("Out of bounds access")?
                    as u32;
                uncompressed_file_size ^= *name
                    .get(name.len() >> 3)
                    .context("Out of bounds access")?
                    as u32;
            }
            file_name
        } else if flags & 4 >= 1 {
            format!("{:X}", crc64)
        } else {
            extractable = false;
            "".to_string()
        });
        Ok((
            Acv1Entry {
                crc64,
                flags,
                file_offset,
                file_size,
                uncompressed_file_size,
                full_path,
                extractable,
            },
            21,
        ))
    }
}

impl Acv1Entry {
    fn dump_entry(&self, file: &RandomAccessFile) -> anyhow::Result<Bytes> {
        let mut buf = BytesMut::new();
        buf.resize(self.file_size as usize, 0);
        file.read_exact_at(self.file_offset as u64, &mut buf)?;

        if self.flags == 0 {
            return Ok(buf.freeze());
        }
        if self.flags & 2 == 0 {
            let name = SHIFT_JIS
                .encode(&self.full_path.to_str().context("Not valid UTF-8")?)
                .0;
            let result = self.file_size as usize / name.len();
            let mut index = 0_usize;
            let mut name_index = 0_usize;
            while index <= self.file_size as usize
                && name_index < name.len() - 1
            {
                for _ in 0..result {
                    *buf.get_mut(index).context("Out of bounds access")? ^=
                        *name
                            .get(name_index)
                            .context("Out of bounds access")?;
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
        Ok(Bytes::from(zlib_decompress(&buf)?))
    }
    fn dump_script(
        &self,
        file: &RandomAccessFile,
        script_key: u32,
    ) -> anyhow::Result<Bytes> {
        let mut buf = BytesMut::new();
        buf.resize(self.file_size as usize, 0);
        file.read_exact_at(self.file_offset as u64, &mut buf)?;

        let xor_key = self.crc64 as u32 ^ script_key;
        buf.chunks_exact_mut(4).for_each(|c| {
            c[0] ^= xor_key as u8;
            c[1] ^= (xor_key >> 8) as u8;
            c[2] ^= (xor_key >> 16) as u8;
            c[3] ^= (xor_key >> 24) as u8;
        });

        Ok(Bytes::from(zlib_decompress(&buf)?))
    }
}
