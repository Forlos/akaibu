use crate::archive;

use super::Scheme;
use anyhow::Context;
use bytes::{Bytes, BytesMut};
use encoding_rs::SHIFT_JIS;
use positioned_io::{RandomAccessFile, ReadAt};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use scroll::{ctx, Pread, LE};
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

const KEY: u32 = 0x65AC9365;
const FILE_ENTRY_SIZE: usize = 12;

#[derive(Debug, Clone)]
pub enum EscArc2Scheme {
    Universal,
}

impl Scheme for EscArc2Scheme {
    fn extract(
        &self,
        file_path: &Path,
    ) -> anyhow::Result<(
        Box<dyn crate::archive::Archive + Sync>,
        crate::archive::NavigableDirectory,
    )> {
        let mut buf = vec![0; 20];
        let file = RandomAccessFile::open(file_path)?;
        file.read_exact_at(0, &mut buf)?;

        let header = buf.pread::<EscArc2Header>(0)?;
        log::debug!("Header: {:#?}", header);

        let mut file_entries =
            vec![0; header.file_count as usize * FILE_ENTRY_SIZE];
        file.read_exact_at(20, &mut file_entries)?;

        let mut file_name_table = vec![0; header.file_name_table_size as usize];
        file.read_exact_at(
            file_entries.len() as u64 + 20,
            &mut file_name_table,
        )?;
        let file_entries = decrypt_file_entries(
            &mut file_entries,
            header.file_entry_key,
            &file_name_table,
        )?;
        let archive = EscArc2 {
            header,
            file_entries,
        };
        log::debug!("Archive: {:#?}", archive);

        let root_dir = EscArc2Archive::new_root_dir(&archive.file_entries);
        let navigable_dir = archive::NavigableDirectory::new(root_dir);
        Ok((Box::new(EscArc2Archive { file, archive }), navigable_dir))
    }

    fn get_name(&self) -> String {
        format!(
            "[EscArc2] {}",
            match self {
                Self::Universal => "EscArc2",
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
struct EscArc2Archive {
    file: RandomAccessFile,
    archive: EscArc2,
}

impl archive::Archive for EscArc2Archive {
    fn extract(
        &self,
        entry: &archive::FileEntry,
    ) -> anyhow::Result<bytes::Bytes> {
        self.archive
            .file_entries
            .iter()
            .find(|e| e.full_path == entry.full_path)
            .map(|e| self.extract(e))
            .context("File not found")?
    }

    fn extract_all(&self, output_path: &Path) -> anyhow::Result<()> {
        self.archive.file_entries.par_iter().try_for_each(
            |entry| -> Result<(), anyhow::Error> {
                let buf = self.extract(entry)?;
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
                File::create(output_file_name)?.write_all(&buf)?;
                Ok(())
            },
        )
    }
}

impl EscArc2Archive {
    fn new_root_dir(entries: &[EscArc2FileEntry]) -> archive::Directory {
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
    fn extract(&self, entry: &EscArc2FileEntry) -> anyhow::Result<Bytes> {
        let mut buf = BytesMut::with_capacity(entry.file_size as usize);
        buf.resize(entry.file_size as usize, 0);
        self.file
            .read_exact_at(entry.file_offset as u64, &mut buf)?;
        Ok(buf.freeze())
    }
}

#[derive(Debug)]
struct EscArc2 {
    header: EscArc2Header,
    file_entries: Vec<EscArc2FileEntry>,
}

#[derive(Debug, Clone, Copy)]
struct EscArc2Header {
    file_count: u32,
    file_entry_key: u32,
    file_name_table_size: u32,
}

impl<'a> ctx::TryFromCtx<'a, ()> for EscArc2Header {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        _ctx: (),
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 8;
        let unk1 = buf.gread_with(off, LE)?;
        let file_count = buf.gread_with(off, LE)?;
        let unk2 = buf.gread_with(off, LE)?;
        Ok((Self::decrypt_header(unk1, file_count, unk2), *off))
    }
}

impl EscArc2Header {
    fn decrypt_header(
        mut unk1: u32,
        mut file_count: u32,
        unk2: u32,
    ) -> EscArc2Header {
        unk1 ^= KEY;
        let mut file_name_table_size = ((unk1 >> 1) ^ unk1) >> 3;
        let mut d = unk1.wrapping_add(unk1) ^ unk1;
        d = d.wrapping_add(d);
        d = d.wrapping_add(d);
        d = d.wrapping_add(d);
        file_name_table_size ^= d ^ unk1;
        file_count ^= file_name_table_size;
        file_name_table_size ^= KEY;
        unk1 = file_name_table_size.wrapping_add(file_name_table_size)
            ^ file_name_table_size;
        unk1 = unk1.wrapping_add(unk1);
        unk1 = unk1.wrapping_add(unk1);
        unk1 = unk1.wrapping_add(unk1);
        let mut file_entry_key =
            ((file_name_table_size >> 1) ^ file_name_table_size) >> 3;
        file_entry_key ^= unk1 ^ file_name_table_size;
        file_name_table_size = unk2 ^ file_entry_key;
        Self {
            file_count,
            file_entry_key,
            file_name_table_size,
        }
    }
}

#[derive(Debug)]
struct EscArc2FileEntry {
    file_offset: u32,
    file_size: u32,
    file_name: String,
    full_path: PathBuf,
}

impl<'a> ctx::TryFromCtx<'a, &[u8]> for EscArc2FileEntry {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        file_name_table: &[u8],
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let file_name_table_offset = buf.gread_with::<u32>(off, LE)? as usize;
        let file_offset = buf.gread_with::<u32>(off, LE)?;
        let file_size = buf.gread_with::<u32>(off, LE)?;
        let full_path = PathBuf::from(
            SHIFT_JIS
                .decode(
                    &file_name_table
                        .get(file_name_table_offset..)
                        .context("Out of bounds read")?
                        .iter()
                        .take_while(|b| **b != 0)
                        .copied()
                        .collect::<Vec<u8>>(),
                )
                .0
                .to_string()
                .replace("\\", "/"),
        );
        let file_name = full_path
            .file_name()
            .context("Could not get file name")?
            .to_str()
            .context("Not valid UTF-8")?
            .to_string();
        Ok((
            Self {
                file_offset,
                file_size,
                file_name,
                full_path,
            },
            *off,
        ))
    }
}

fn decrypt_file_entries(
    file_entries: &mut [u8],
    mut file_entry_key: u32,
    file_name_table: &[u8],
) -> anyhow::Result<Vec<EscArc2FileEntry>> {
    file_entries.chunks_exact_mut(4).for_each(|chunk| {
        file_entry_key ^= KEY;
        let mut d = file_entry_key.wrapping_add(file_entry_key);
        d ^= file_entry_key;
        let mut c = file_entry_key;
        c >>= 1;
        d = d.wrapping_add(d);
        c ^= file_entry_key;
        d = d.wrapping_add(d);
        c >>= 3;
        d = d.wrapping_add(d);
        c ^= d;
        file_entry_key ^= c;
        chunk[0] ^= file_entry_key as u8;
        chunk[1] ^= (file_entry_key >> 8) as u8;
        chunk[2] ^= (file_entry_key >> 16) as u8;
        chunk[3] ^= (file_entry_key >> 24) as u8;
    });
    file_entries
        .chunks_exact(12)
        .try_fold(Vec::new(), |mut v, chunk| {
            v.push(chunk.pread_with::<EscArc2FileEntry>(0, file_name_table)?);
            Ok(v)
        })
}
