use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

use super::Scheme;
use crate::archive::{self, FileContents, NavigableDirectory};
use anyhow::Context;
use bytes::BytesMut;
use positioned_io::RandomAccessFile;
use positioned_io_preview::ReadAt;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use scroll::{ctx, Pread, LE};

#[derive(Debug, Clone)]
pub enum ArcScheme {
    Universal,
}

impl Scheme for ArcScheme {
    fn extract(
        &self,
        file_path: &Path,
    ) -> anyhow::Result<(
        Box<dyn crate::archive::Archive + Sync>,
        NavigableDirectory,
    )> {
        let mut buf = vec![0; 8];
        let file = RandomAccessFile::open(file_path)?;
        file.read_exact_at(0, &mut buf)?;
        let header = buf.pread_with::<ArcHeader>(0, LE)?;
        log::debug!("Header: {:#?}", header);

        let mut file_entries = Vec::with_capacity(header.entry_count as usize);
        buf.resize(header.entries_size as usize, 0);
        file.read_exact_at(8, &mut buf)?;

        let off = &mut 0;
        for _ in 0..header.entry_count {
            file_entries.push(buf.gread::<ArcFileEntry>(off)?);
        }

        let root_dir = ArcArchive::new_root_dir(&file_entries);
        let navigable_dir = archive::NavigableDirectory::new(root_dir);
        Ok((
            Box::new(ArcArchive {
                file,
                header,
                file_entries,
            }),
            navigable_dir,
        ))
    }

    fn get_name(&self) -> String {
        format!(
            "[WILLPLUS ARC] {}",
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
struct ArcArchive {
    file: RandomAccessFile,
    header: ArcHeader,
    file_entries: Vec<ArcFileEntry>,
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

    fn extract_all(&self, output_path: &Path) -> anyhow::Result<()> {
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
    fn extract(&self, entry: &ArcFileEntry) -> anyhow::Result<FileContents> {
        let mut buf = BytesMut::with_capacity(entry.file_size as usize);
        buf.resize(entry.file_size as usize, 0);

        self.file.read_exact_at(
            8 + self.header.entries_size as u64 + entry.file_offset as u64,
            &mut buf,
        )?;
        Ok(FileContents {
            contents: buf.freeze(),
            type_hint: None,
        })
    }
}

#[derive(Debug, Pread, Copy, Clone)]
struct ArcHeader {
    entry_count: u32,
    entries_size: u32,
}

#[derive(Debug)]
struct ArcFileEntry {
    file_size: u32,
    file_offset: u64,
    full_path: PathBuf,
}

impl<'a> ctx::TryFromCtx<'a, ()> for ArcFileEntry {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        _ctx: (),
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let file_size = buf.gread_with::<u32>(off, LE)?;
        let file_offset = buf.gread_with::<u32>(off, LE)? as u64;
        let name = &buf[*off..]
            .chunks_exact(2)
            .take_while(|c| !(c[0] == 0 && c[1] == 0))
            .map(|c| c[0] as u16 + ((c[1] as u16) << 8))
            .collect::<Vec<u16>>();
        let full_path = PathBuf::from(String::from_utf16(name)?);
        *off += name.len() * 2 + 2;
        Ok((
            ArcFileEntry {
                file_size,
                file_offset,
                full_path,
            },
            *off,
        ))
    }
}
