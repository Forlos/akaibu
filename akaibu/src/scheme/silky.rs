use crate::archive::{self, FileContents};

use super::Scheme;
use anyhow::Context;
use bytes::{Bytes, BytesMut};
use encoding_rs::SHIFT_JIS;
use positioned_io::{RandomAccessFile, ReadAt};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use scroll::{ctx, Pread, BE, LE};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub enum SilkyScheme {
    Universal,
}

impl Scheme for SilkyScheme {
    fn extract(
        &self,
        file_path: &Path,
    ) -> anyhow::Result<(
        Box<dyn crate::archive::Archive>,
        crate::archive::NavigableDirectory,
    )> {
        let mut buf = vec![0; 4];
        let file = RandomAccessFile::open(file_path)?;
        file.read_exact_at(0, &mut buf)?;
        let entries_size = buf.pread_with::<u32>(0, LE)? as usize;

        let mut buf = vec![0; entries_size];
        file.read_exact_at(4, &mut buf)?;

        let off = &mut 0;
        let mut entries = Vec::new();
        while *off < entries_size {
            entries.push(buf.gread(off)?);
        }
        let archive = Silky { entries };
        log::debug!("Archive: {:#?}", archive);

        let root_dir = SilkyArchive::new_root_dir(&archive.entries);
        let navigable_dir = archive::NavigableDirectory::new(root_dir);
        Ok((Box::new(SilkyArchive { file, archive }), navigable_dir))
    }

    fn get_name(&self) -> String {
        format!(
            "[SILKY] {}",
            match self {
                Self::Universal => "Universal",
            }
        )
    }

    fn get_schemes() -> Vec<Box<dyn Scheme>>
    where
        Self: Sized,
    {
        vec![Box::new(Self::Universal)]
    }
}

#[derive(Debug)]
struct SilkyArchive {
    file: RandomAccessFile,
    archive: Silky,
}

impl archive::Archive for SilkyArchive {
    fn extract(
        &self,
        entry: &archive::FileEntry,
    ) -> anyhow::Result<FileContents> {
        self.archive
            .entries
            .iter()
            .find(|e| e.full_path == entry.full_path)
            .map(|e| self.extract(e))
            .context("File not found")?
    }

    fn extract_all(&self, output_path: &Path) -> anyhow::Result<()> {
        self.archive.entries.par_iter().try_for_each(|entry| {
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

impl SilkyArchive {
    fn new_root_dir(entries: &[SilkyEntry]) -> archive::Directory {
        archive::Directory::new(
            entries
                .iter()
                .map(|entry| {
                    let file_offset = entry.file_offset as u64;
                    let file_size = entry.file_size as u64;
                    archive::FileEntry {
                        file_name: entry.file_name.clone(),
                        full_path: entry.full_path.clone(),
                        file_offset,
                        file_size,
                    }
                })
                .collect(),
        )
    }
    fn extract(&self, entry: &SilkyEntry) -> anyhow::Result<FileContents> {
        let mut buf = BytesMut::with_capacity(entry.file_size as usize);
        buf.resize(entry.file_size as usize, 0);
        self.file.read_exact_at(entry.file_offset, &mut buf)?;
        let contents = if entry.uncompressed_file_size > entry.file_size {
            decompress(&buf, entry.uncompressed_file_size as usize)
        } else {
            buf.freeze()
        };
        Ok(FileContents {
            contents,
            type_hint: None,
        })
    }
}

#[derive(Debug)]
struct Silky {
    entries: Vec<SilkyEntry>,
}

#[derive(Debug)]
struct SilkyEntry {
    file_offset: u64,
    file_size: u32,
    uncompressed_file_size: u32,
    file_name: String,
    full_path: PathBuf,
}

impl<'a> ctx::TryFromCtx<'a, ()> for SilkyEntry {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        _: (),
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let name_length = buf.gread::<u8>(off)?;

        let file_name = SHIFT_JIS
            .decode(
                &buf.get(*off..*off + name_length as usize)
                    .context("Out of bounds read")?
                    .iter()
                    .enumerate()
                    .map(|(i, b)| b.wrapping_add(name_length - i as u8))
                    .collect::<Vec<u8>>(),
            )
            .0
            .to_string();
        *off += name_length as usize;
        let full_path = PathBuf::from(&file_name);
        let file_size = buf.gread_with::<u32>(off, BE)?;
        let uncompressed_file_size = buf.gread_with::<u32>(off, BE)?;
        let file_offset = buf.gread_with::<u32>(off, BE)? as u64;
        Ok((
            Self {
                file_offset,
                file_size,
                uncompressed_file_size,
                file_name,
                full_path,
            },
            *off,
        ))
    }
}

fn decompress(buf: &[u8], dest_len: usize) -> Bytes {
    let mut dest = vec![0u8; dest_len];
    let mut lookup_table = vec![0u8; 4096];

    let mut x = 0_u16;
    let mut lookup_index = 4078;
    let mut bytes_read = 0;
    let mut bytes_written = 0;
    while bytes_read < buf.len() {
        x >>= 1;
        if (x & 0x100) == 0 {
            x = buf[bytes_read] as u16;
            bytes_read += 1;
            x |= 0xFF00;
        }
        if ((x & 0xFF) & 1) == 0 {
            let bl = buf[bytes_read];
            bytes_read += 1;
            let cl = buf[bytes_read];
            bytes_read += 1;
            let mut s = cl as u16;
            let mut d = s as u16;
            let mut c = bl as u16;
            d &= 0xF0;
            s &= 0x0F;
            d <<= 4;
            s += 3;
            d |= c;
            c = s;
            if c > 0 {
                s = d;
                let mut counter = c;
                while counter != 0 {
                    c = s;
                    s += 1;
                    c &= 0xFFF;
                    d = lookup_table[c as usize] as u16;
                    dest[bytes_written] = d as u8;
                    c = lookup_index;
                    bytes_written += 1;
                    lookup_index += 1;
                    lookup_index &= 0xFFF;
                    lookup_table[c as usize] = d as u8;

                    counter -= 1;
                }
            }
        } else {
            let d = buf[bytes_read];
            bytes_read += 1;
            dest[bytes_written] = d;
            bytes_written += 1;
            let c = lookup_index;
            lookup_index += 1;
            lookup_index &= 0xFFF;
            lookup_table[c as usize] = d;
        }
    }
    Bytes::from(dest)
}
