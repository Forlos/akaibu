use super::Scheme;
use crate::archive;
use anyhow::Context;
use bytes::BytesMut;
use positioned_io::{RandomAccessFile, ReadAt};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use scroll::{ctx, Pread, LE};
use std::{fs::File, io::Write, path::PathBuf};

#[derive(Debug, Clone)]
pub enum Pf8Scheme {
    Universal,
}

impl Scheme for Pf8Scheme {
    fn extract(
        &self,
        file_path: &PathBuf,
    ) -> anyhow::Result<(
        Box<dyn crate::archive::Archive + Sync>,
        archive::NavigableDirectory,
    )> {
        let mut buf = vec![0; 11];
        let file = RandomAccessFile::open(file_path)?;
        file.read_exact_at(0, &mut buf)?;

        let header = buf.pread::<Pf8Header>(0)?;
        log::debug!("Header: {:#?}", header);

        let mut buf = vec![0; header.archive_data_size as usize - 4];
        file.read_exact_at(11, &mut buf)?;
        let archive = buf.pread_with::<Pf8>(0, header)?;
        log::debug!("Archive: {:#?}", archive);

        let mut buf = vec![0; header.archive_data_size as usize];
        file.read_exact_at(7, &mut buf)?;
        let sha1 = sha1::Sha1::from(&buf).digest().bytes();

        let root_dir = Pf8Archive::new_root_dir(&archive.file_entries);
        let navigable_dir = archive::NavigableDirectory::new(root_dir);
        Ok((
            Box::new(Pf8Archive {
                file,
                sha1,
                archive,
            }),
            navigable_dir,
        ))
    }
    fn get_name(&self) -> String {
        format!(
            "[PF8] {}",
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
struct Pf8Archive {
    file: RandomAccessFile,
    sha1: [u8; 20],
    archive: Pf8,
}

impl archive::Archive for Pf8Archive {
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

    fn extract_all(&self, output_path: &PathBuf) -> anyhow::Result<()> {
        self.archive.file_entries.par_iter().try_for_each(|entry| {
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
        })
    }
}

impl Pf8Archive {
    fn new_root_dir(entries: &[Pf8FileEntry]) -> archive::Directory {
        archive::Directory::new(
            entries
                .iter()
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
    fn extract(&self, entry: &Pf8FileEntry) -> anyhow::Result<bytes::Bytes> {
        let mut buf = BytesMut::with_capacity(entry.file_size as usize);
        buf.resize(entry.file_size as usize, 0);

        self.file
            .read_exact_at(entry.file_offset as u64, &mut buf)?;
        self.decrypt_file(&mut buf)?;
        Ok(buf.freeze())
    }
    fn decrypt_file(&self, data: &mut [u8]) -> anyhow::Result<()> {
        data.iter_mut().enumerate().try_for_each(|(i, b)| {
            *b ^= self
                .sha1
                .get(i % self.sha1.len())
                .context("Out of bounds access")?;
            Ok(())
        })
    }
}

#[derive(Debug)]
struct Pf8 {
    header: Pf8Header,
    file_entries: Vec<Pf8FileEntry>,
}

impl<'a> ctx::TryFromCtx<'a, Pf8Header> for Pf8 {
    type Error = anyhow::Error;
    fn try_from_ctx(
        buf: &'a [u8],
        header: Pf8Header,
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let mut file_entries =
            Vec::with_capacity(header.file_entries_count as usize);
        for _ in 0..header.file_entries_count {
            file_entries.push(buf.gread(off)?);
        }
        Ok((
            Pf8 {
                header,
                file_entries,
            },
            *off,
        ))
    }
}

#[derive(Debug, Pread, Copy, Clone)]
struct Pf8Header {
    magic: [u8; 2],
    version: u8,
    archive_data_size: u32,
    file_entries_count: u32,
}

#[derive(Debug)]
struct Pf8FileEntry {
    file_name_size: u32,
    full_path: PathBuf,
    unk: u32,
    file_offset: u32,
    file_size: u32,
}

impl<'a> ctx::TryFromCtx<'a, ()> for Pf8FileEntry {
    type Error = anyhow::Error;
    fn try_from_ctx(
        buf: &'a [u8],
        _: (),
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let file_name_size = buf.gread_with::<u32>(off, LE)?;
        let full_path = PathBuf::from(
            String::from_utf8(
                buf.get(*off..*off + file_name_size as usize)
                    .context("Out of bounds access")?
                    .to_vec(),
            )?
            .replace("\\", "/"),
        );
        *off += file_name_size as usize;
        let unk = buf.gread_with::<u32>(off, LE)?;
        let file_offset = buf.gread_with::<u32>(off, LE)?;
        let file_size = buf.gread_with::<u32>(off, LE)?;
        Ok((
            Pf8FileEntry {
                file_name_size,
                full_path,
                unk,
                file_offset,
                file_size,
            },
            *off,
        ))
    }
}
