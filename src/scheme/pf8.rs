use super::Scheme;
use crate::archive;
use anyhow::Context;
use bytes::BytesMut;
use scroll::{ctx, Pread, LE};
use std::{
    fs::File,
    io::{Read, Seek},
    path::PathBuf,
};

#[derive(Debug)]
pub enum Pf8Scheme {
    Universal,
}

impl Scheme for Pf8Scheme {
    fn extract(
        &self,
        file_path: &std::path::PathBuf,
    ) -> anyhow::Result<Box<dyn crate::archive::Archive + Sync>> {
        let mut buf = vec![0; 11];
        let mut file = File::open(file_path)?;
        file.read_exact(&mut buf)?;

        let header = buf.pread::<Pf8Header>(0)?;
        log::debug!("Header: {:#?}", header);

        let mut buf = vec![0; header.archive_data_size as usize - 4];
        file.read_exact(&mut buf)?;
        let pf8 = buf.pread_with(0, header)?;
        log::debug!("Archive: {:#?}", pf8);

        let mut buf = vec![0; header.archive_data_size as usize];
        file.seek(std::io::SeekFrom::Start(7))?;
        file.read_exact(&mut buf)?;
        let sha1 = sha1::Sha1::from(&buf).digest().bytes();

        Ok(Box::new(Pf8Archive {
            file_path: file_path.clone(),
            sha1,
            archive: pf8,
        }))
    }
    fn get_name(&self) -> &str {
        "pf8"
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
    file_path: PathBuf,
    sha1: [u8; 20],
    archive: Pf8,
}

impl archive::Archive for Pf8Archive {
    fn get_files(&self) -> Vec<archive::FileEntry> {
        self.archive
            .file_entries
            .iter()
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
            .find(|e| e.file_name == entry.file_name)
            .map(|e| self.extract(e))
            .context("File not found")?
    }
}

impl Pf8Archive {
    fn extract(&self, entry: &Pf8FileEntry) -> anyhow::Result<bytes::Bytes> {
        let mut file = File::open(&self.file_path)?;
        file.seek(std::io::SeekFrom::Start(entry.file_offset as u64))?;
        let mut buf = BytesMut::with_capacity(entry.file_size as usize);
        buf.resize(entry.file_size as usize, 0);

        file.read_exact(&mut buf)?;
        self.decrypt_file(&mut buf);
        Ok(buf.freeze())
    }
    fn decrypt_file(&self, data: &mut [u8]) {
        data.iter_mut().enumerate().for_each(|(i, b)| {
            *b ^= self.sha1[i % self.sha1.len()];
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
    file_name: String,
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
        let file_name = String::from_utf8(
            buf[*off..*off + file_name_size as usize].to_vec(),
        )?
        .replace("\\", "/");
        *off += file_name_size as usize;
        let unk = buf.gread_with::<u32>(off, LE)?;
        let file_offset = buf.gread_with::<u32>(off, LE)?;
        let file_size = buf.gread_with::<u32>(off, LE)?;
        Ok((
            Pf8FileEntry {
                file_name_size,
                file_name,
                unk,
                file_offset,
                file_size,
            },
            *off,
        ))
    }
}
