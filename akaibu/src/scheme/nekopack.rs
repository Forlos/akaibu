use crate::{
    archive::{self, FileContents},
    util::zlib_decompress,
};
use anyhow::Context;
use bytes::{Bytes, BytesMut};
use encoding_rs::SHIFT_JIS;
use positioned_io::{RandomAccessFile, ReadAt};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use scroll::{ctx, Pread, LE};
use std::{fs::File, io::Write, path::PathBuf};

use super::Scheme;

#[derive(Debug, Clone)]
pub enum PackScheme {
    Universal,
}

impl Scheme for PackScheme {
    fn extract(
        &self,
        file_path: &std::path::Path,
    ) -> anyhow::Result<(
        Box<dyn crate::archive::Archive + Sync>,
        crate::archive::NavigableDirectory,
    )> {
        let mut buf = vec![0; 14];
        let file = RandomAccessFile::open(file_path)?;
        file.read_exact_at(0, &mut buf)?;
        let header = buf.pread_with::<PackHeader>(0, LE)?;
        log::debug!("Header: {:#?}", header);

        let mut file_entries = Vec::new();
        let off = &mut 0;

        let mut buf = vec![0; header.entries_size as usize - 4];
        file.read_exact_at(14, &mut buf)?;
        while *off < header.entries_size as usize - 4 {
            file_entries.push(buf.gread(off)?);
        }

        let root_dir = PackArchive::new_root_dir(&file_entries);
        let navigable_dir = archive::NavigableDirectory::new(root_dir);
        Ok((Box::new(PackArchive { file, file_entries }), navigable_dir))
    }

    fn get_name(&self) -> String {
        format!(
            "[NEKOPACK ARC] {}",
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
struct PackArchive {
    file: RandomAccessFile,
    file_entries: Vec<PackFileEntry>,
}

impl archive::Archive for PackArchive {
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

impl PackArchive {
    fn new_root_dir(entries: &[PackFileEntry]) -> archive::Directory {
        archive::Directory::new(
            entries
                .iter()
                .map(|entry| {
                    let file_offset = entry.file_offset as u64;
                    let file_size = entry.file_size as u64;
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
    fn extract(&self, entry: &PackFileEntry) -> anyhow::Result<FileContents> {
        let mut buf = BytesMut::with_capacity(entry.file_size as usize);
        buf.resize(entry.file_size as usize, 0);
        self.file.read_exact_at(entry.file_offset, &mut buf)?;

        let contents = decompress(&mut buf)?;

        Ok(FileContents {
            contents,
            type_hint: None,
        })
    }
}

#[derive(Debug, Pread)]
struct PackHeader {
    magic: [u8; 8],
    version: [u8; 2],
    entries_size: u32,
}

#[derive(Debug)]
struct PackFileEntry {
    file_size: u32,
    file_offset: u64,
    full_path: PathBuf,
}

impl<'a> ctx::TryFromCtx<'a, ()> for PackFileEntry {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        _ctx: (),
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let name_size = buf.gread_with::<u32>(off, LE)? as usize;
        let full_path = PathBuf::from(
            SHIFT_JIS
                .decode(&buf[*off..*off + name_size - 1])
                .0
                .replace("\\", "/"),
        );
        let file_name_sum: u32 =
            buf[*off..*off + name_size].iter().map(|b| *b as u32).sum();
        *off += name_size;
        let file_offset =
            (buf.gread_with::<u32>(off, LE)? ^ file_name_sum) as u64;
        let file_size = buf.gread_with::<u32>(off, LE)? ^ file_name_sum;
        Ok((
            Self {
                file_size,
                file_offset,
                full_path,
            },
            *off,
        ))
    }
}

fn decompress(src: &mut [u8]) -> anyhow::Result<Bytes> {
    let mut s = ((src.len() >> 3) as u8).wrapping_add(34);
    if src.len() > 32 {
        for i in 0..32 {
            src[i] ^= s;
            s <<= 3;
        }
    }
    Ok(Bytes::from(zlib_decompress(&src[..src.len() - 4])?))
}
