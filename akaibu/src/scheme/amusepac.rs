use super::Scheme;
use crate::archive::{self, FileContents};
use anyhow::Context;
use bytes::BytesMut;
use positioned_io_preview::{RandomAccessFile, ReadAt};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use scroll::{ctx, Pread, LE};
use std::{fs::File, io::Write, path::PathBuf};

#[derive(Debug, Clone)]
pub enum PacScheme {
    Universal,
}

impl Scheme for PacScheme {
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
        let header = buf.pread_with::<PacHeader>(0, LE)?;
        log::debug!("Header: {:#?}", header);

        let mut file_entries =
            Vec::with_capacity(header.entries_count as usize);
        let off = &mut 0;

        let mut buf = vec![0; header.entries_count as usize * 0x28];
        file.read_exact_at(0x804, &mut buf)?;
        for _ in 0..header.entries_count {
            file_entries.push(buf.gread(off)?);
        }

        let root_dir = PacArchive::new_root_dir(&file_entries);
        let navigable_dir = archive::NavigableDirectory::new(root_dir);
        Ok((Box::new(PacArchive { file, file_entries }), navigable_dir))
    }

    fn get_name(&self) -> String {
        format!(
            "[AMUSE PAC] {}",
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
struct PacArchive {
    file: RandomAccessFile,
    file_entries: Vec<PacFileEntry>,
}

impl archive::Archive for PacArchive {
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

impl PacArchive {
    fn new_root_dir(entries: &[PacFileEntry]) -> archive::Directory {
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
    fn extract(&self, entry: &PacFileEntry) -> anyhow::Result<FileContents> {
        let mut buf = BytesMut::with_capacity(entry.file_size as usize);
        buf.resize(entry.file_size as usize, 0);
        self.file.read_exact_at(entry.file_offset, &mut buf)?;

        Ok(FileContents {
            contents: buf.freeze(),
            type_hint: None,
        })
    }
}

#[derive(Debug, Pread)]
struct PacHeader {
    magic: [u8; 4],
    unk0: u32,
    entries_count: u32,
}

#[derive(Debug)]
struct PacFileEntry {
    file_size: u32,
    file_offset: u64,
    full_path: PathBuf,
}

impl<'a> ctx::TryFromCtx<'a, ()> for PacFileEntry {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        _ctx: (),
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 32;
        let full_path = PathBuf::from(String::from_utf8(
            buf[0..32]
                .iter()
                .take_while(|b| **b != 0)
                .map(|b| *b)
                .collect(),
        )?);
        let file_size = buf.gread_with::<u32>(off, LE)?;
        let file_offset = buf.gread_with::<u32>(off, LE)? as u64;
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
