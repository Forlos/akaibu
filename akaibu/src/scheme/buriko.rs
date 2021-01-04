use super::Scheme;
use crate::archive;
use anyhow::Context;
use bytes::Bytes;
use bytes::BytesMut;
use encoding_rs::SHIFT_JIS;
use positioned_io::{RandomAccessFile, ReadAt};
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use scroll::ctx;
use scroll::Pread;
use scroll::LE;
use std::convert::TryInto;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

const BURIKO_ENTRY_SIZE: usize = 0x80;
const BURIKO_ENTRY_NAME_SIZE: usize = 0x60;
const SOUND_FILE_MAGIC: &[u8] = b"bw  ";

#[derive(Debug, Clone)]
pub enum BurikoScheme {
    Universal,
}

impl Scheme for BurikoScheme {
    fn extract(
        &self,
        file_path: &std::path::PathBuf,
    ) -> anyhow::Result<(
        Box<dyn crate::archive::Archive + Sync>,
        crate::archive::NavigableDirectory,
    )> {
        let mut buf = vec![0; 16];
        let file = RandomAccessFile::open(file_path)?;
        file.read_exact_at(0, &mut buf)?;

        let header = buf.pread::<BurikoHeader>(0)?;
        log::debug!("Header: {:#?}", header);

        let mut buf = vec![0; header.entry_count as usize * BURIKO_ENTRY_SIZE];
        file.read_exact_at(16, &mut buf)?;
        let archive = buf.pread_with::<Buriko>(0, header)?;
        log::debug!("Archive: {:#?}", archive);

        let root_dir = BurikoArchive::new_root_dir(&archive.file_entries);
        let navigable_dir = archive::NavigableDirectory::new(root_dir);
        Ok((Box::new(BurikoArchive { file, archive }), navigable_dir))
    }

    fn get_name(&self) -> String {
        format!(
            "[BURIKO] {}",
            match self {
                Self::Universal => "Buriko",
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
struct BurikoArchive {
    file: RandomAccessFile,
    archive: Buriko,
}

impl archive::Archive for BurikoArchive {
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

    fn extract_all(
        &self,
        output_path: &std::path::PathBuf,
    ) -> anyhow::Result<()> {
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

impl BurikoArchive {
    fn new_root_dir(entries: &[BurikoFileEntry]) -> archive::Directory {
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
    fn extract(&self, entry: &BurikoFileEntry) -> anyhow::Result<Bytes> {
        let mut buf = BytesMut::with_capacity(entry.file_size as usize);
        buf.resize(entry.file_size as usize, 0);
        self.file.read_exact_at(
            self.archive.header.file_contents_offset + entry.file_offset as u64,
            &mut buf,
        )?;
        if buf.get(4..8).context("Out of bounds access")? == SOUND_FILE_MAGIC {
            buf = buf.split_off(0x40);
        }
        Ok(buf.freeze())
    }
}

#[derive(Debug)]
struct Buriko {
    header: BurikoHeader,
    file_entries: Vec<BurikoFileEntry>,
}

impl<'a> ctx::TryFromCtx<'a, BurikoHeader> for Buriko {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        header: BurikoHeader,
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let mut file_entries = Vec::with_capacity(header.entry_count as usize);
        for _ in 0..header.entry_count {
            file_entries.push(buf.gread(off)?);
        }
        Ok((
            Self {
                header,
                file_entries,
            },
            *off,
        ))
    }
}

#[derive(Debug, Copy, Clone)]
struct BurikoHeader {
    magic: [u8; 10],
    version: u16,
    entry_count: u32,
    file_contents_offset: u64,
}

impl<'a> ctx::TryFromCtx<'a, scroll::Endian> for BurikoHeader {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        _: scroll::Endian,
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let magic: [u8; 10] =
            buf.get(0..10).context("Out of bounds access")?.try_into()?;
        *off += magic.len();
        let version = String::from_utf8(
            buf.get(*off..*off + 2)
                .context("Out of bounds access")?
                .to_vec(),
        )?
        .parse()?;
        *off += 2;
        let entry_count = buf.gread_with::<u32>(off, LE)?;
        let file_contents_offset =
            0x10 + entry_count as u64 * BURIKO_ENTRY_SIZE as u64;
        Ok((
            Self {
                magic,
                version,
                entry_count,
                file_contents_offset,
            },
            *off,
        ))
    }
}

#[derive(Debug)]
struct BurikoFileEntry {
    full_path: PathBuf,
    file_offset: u32,
    file_size: u32,
    unknown: [u8; 18],
}

impl<'a> ctx::TryFromCtx<'a, scroll::Endian> for BurikoFileEntry {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        _: scroll::Endian,
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let full_path = PathBuf::from(
            SHIFT_JIS
                .decode(
                    buf.get(*off..*off + BURIKO_ENTRY_NAME_SIZE)
                        .context("Out of bounds access")?
                        .split(|b| *b == 0)
                        .next()
                        .context("Could not split")?,
                )
                .0
                .to_string(),
        );
        *off += BURIKO_ENTRY_NAME_SIZE;
        let file_offset = buf.gread_with::<u32>(off, LE)?;
        let file_size = buf.gread_with::<u32>(off, LE)?;
        let unknown: [u8; 18] = buf
            .get(*off..*off + 18)
            .context("Out of bounds access")?
            .try_into()?;
        *off += unknown.len();
        Ok((
            Self {
                full_path,
                file_offset,
                file_size,
                unknown,
            },
            BURIKO_ENTRY_SIZE,
        ))
    }
}
