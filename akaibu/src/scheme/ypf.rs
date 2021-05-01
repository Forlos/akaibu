use crate::{archive, error::AkaibuError, scheme::Scheme};
use crate::{archive::FileContents, util::zlib_decompress};
use anyhow::Context;
use bytes::Bytes;
use bytes::BytesMut;
use encoding_rs::SHIFT_JIS;
use positioned_io::{RandomAccessFile, ReadAt};
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use scroll::{ctx, Pread, LE};
use std::fs::File;
use std::io::Write;
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Clone)]
pub enum YpfScheme {
    Universal,
}

impl Scheme for YpfScheme {
    fn extract(
        &self,
        file_path: &std::path::Path,
    ) -> anyhow::Result<(
        Box<dyn crate::archive::Archive + Sync>,
        crate::archive::NavigableDirectory,
    )> {
        let mut buf = vec![0; 32];
        let file = RandomAccessFile::open(file_path)?;
        file.read_exact_at(0, &mut buf)?;

        let header = buf.pread::<YpfHeader>(0)?;
        log::debug!("Header: {:#?}", header);

        let decrypt_name_table =
            get_decrypt_name_table(header.archive_version)?;

        let mut buf = vec![0; header.entry_data_size as usize];
        file.read_exact_at(32, &mut buf)?;
        let archive =
            buf.pread_with::<Ypf>(0, (header, &decrypt_name_table))?;
        log::debug!("Archive: {:#?}", archive);

        let root_dir = YpfArchive::new_root_dir(&archive.file_entries);
        let navigable_dir = archive::NavigableDirectory::new(root_dir);
        Ok((Box::new(YpfArchive { file, archive }), navigable_dir))
    }

    fn get_name(&self) -> String {
        format!(
            "[YPF] {}",
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
struct YpfArchive {
    file: RandomAccessFile,
    archive: Ypf,
}

impl archive::Archive for YpfArchive {
    fn extract(
        &self,
        entry: &archive::FileEntry,
    ) -> anyhow::Result<FileContents> {
        self.archive
            .file_entries
            .iter()
            .find(|e| e.full_path == entry.full_path)
            .map(|e| self.extract(e))
            .context("File not found")?
    }

    fn extract_all(&self, output_path: &std::path::Path) -> anyhow::Result<()> {
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

impl YpfArchive {
    fn new_root_dir(entries: &[YpfFileEntry]) -> archive::Directory {
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
    fn extract(&self, entry: &YpfFileEntry) -> anyhow::Result<FileContents> {
        let mut buf = BytesMut::with_capacity(entry.file_size as usize);
        let contents = if entry.flags == 1 {
            buf.resize(entry.compressed_file_size as usize, 0);
            self.file.read_exact_at(entry.file_offset, &mut buf)?;
            Bytes::from(zlib_decompress(&buf)?)
        } else {
            buf.resize(entry.file_size as usize, 0);
            self.file.read_exact_at(entry.file_offset, &mut buf)?;
            buf.freeze()
        };
        Ok(FileContents {
            contents,
            type_hint: None,
        })
    }
}

#[derive(Debug)]
struct Ypf {
    header: YpfHeader,
    file_entries: Vec<YpfFileEntry>,
}

impl<'a> ctx::TryFromCtx<'a, (YpfHeader, &'a [u8])> for Ypf {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        (header, decrypt_name_table): (YpfHeader, &'a [u8]),
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let mut file_entries = Vec::with_capacity(header.entry_count as usize);
        for _ in 0..header.entry_count {
            file_entries
                .push(buf.gread_with(off, (&header, decrypt_name_table))?);
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

#[derive(Debug, Pread, Copy, Clone)]
struct YpfHeader {
    magic: [u8; 4],
    archive_version: u32,
    entry_count: u32,
    entry_data_size: u32,
    padding: [u8; 16],
}

#[derive(Debug)]
struct YpfFileEntry {
    unk0: u32,
    name_size: u8,
    full_path: PathBuf,
    unk1: u8,
    flags: u8,
    file_size: u32,
    compressed_file_size: u32,
    file_offset: u64,
    unk2: u32,
}

impl<'a> ctx::TryFromCtx<'a, (&'a YpfHeader, &'a [u8])> for YpfFileEntry {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        (header, decrypt_name_table): (&'a YpfHeader, &'a [u8]),
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let unk0 = buf.gread_with::<u32>(off, LE)?;
        let name_size =
            get_name_size(buf.gread_with::<u8>(off, LE)?, decrypt_name_table)?;
        let full_path = decrypt_file_name(
            &buf.get(*off..*off + name_size)
                .context("Out of bounds access")?,
            &header,
        );
        *off += name_size;
        let unk1 = buf.gread_with::<u8>(off, LE)?;
        let flags = buf.gread_with::<u8>(off, LE)?;
        let file_size = buf.gread_with::<u32>(off, LE)?;
        let compressed_file_size = buf.gread_with::<u32>(off, LE)?;
        let file_offset = buf.gread_with::<u64>(off, LE)?;
        let unk2 = buf.gread_with::<u32>(off, LE)?;
        Ok((
            Self {
                unk0,
                name_size: name_size as u8,
                full_path,
                unk1,
                flags,
                file_size,
                compressed_file_size,
                file_offset,
                unk2,
            },
            *off,
        ))
    }
}

#[inline]
fn get_name_size(
    name_size: u8,
    decrypt_name_table: &[u8],
) -> anyhow::Result<usize> {
    Ok(*decrypt_name_table
        .get(!name_size as usize)
        .context("Out of bounds context")? as usize)
}

fn get_decrypt_name_table(archive_version: u32) -> anyhow::Result<Vec<u8>> {
    let decrypt_name_tables: HashMap<u32, Vec<u8>> = serde_json::from_slice(
        &crate::Resources::get("ypf/decrypt_name_tables.json").context(
            format!("Could not find file: {}", "ypf/decrypt_name_tables.json"),
        )?,
    )?;
    Ok(match decrypt_name_tables.get(&archive_version) {
        Some(table) => table.clone(),
        None => {
            return Err(AkaibuError::Unimplemented(format!(
                "Unsupported YPF archive version: {}",
                archive_version
            ))
            .into())
        }
    })
}

fn decrypt_file_name(buf: &[u8], header: &YpfHeader) -> PathBuf {
    let mut result: Vec<u8> = buf.iter().map(|b| !b).collect();
    if header.archive_version == 500 {
        result.iter_mut().for_each(|b| *b ^= 0x36);
    }
    PathBuf::from(SHIFT_JIS.decode(&result).0.to_string().replace("\\", "/"))
}
