use super::Scheme;
use crate::{
    archive::{self, FileContents},
    resource::ResourceMagic,
};
use anyhow::Context;
use bytes::BytesMut;
use positioned_io::{RandomAccessFile, ReadAt};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use scroll::{ctx, Pread, LE};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum IarScheme {
    Universal,
}

impl Scheme for IarScheme {
    fn extract(
        &self,
        file_path: &Path,
    ) -> anyhow::Result<(
        Box<dyn crate::archive::Archive + Sync>,
        crate::archive::NavigableDirectory,
    )> {
        let mut buf = vec![0; 28];
        let file = RandomAccessFile::open(file_path)?;
        file.read_exact_at(4, &mut buf)?;
        let header = buf.pread::<IarHeader>(0)?;
        log::debug!("Header: {:#?}", header);
        let mut file_entries = Vec::with_capacity(header.entry_count as usize);

        let mut entry_index_table = vec![0; header.entry_count as usize * 8];
        file.read_exact_at(32, &mut entry_index_table)?;

        buf.resize(72, 0);
        for i in 0..header.entry_count as usize {
            let off = entry_index_table.pread_with::<u64>(i * 8, LE)?;
            file.read_exact_at(off, &mut buf)?;
            let entry = buf.pread_with::<IarFileEntry>(0, (off, i as u64))?;
            if !entry.versions_to_ignore() {
                file_entries.push(entry);
            }
        }
        let archive = Iar {
            header,
            file_entries,
        };
        log::debug!("Archive: {:?}", archive);

        let root_dir = IarArchive::new_root_dir(&archive.file_entries);
        let navigable_dir = archive::NavigableDirectory::new(root_dir);
        Ok((Box::new(IarArchive { file, archive }), navigable_dir))
    }

    fn get_name(&self) -> String {
        format!(
            "[IAR] {}",
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
struct IarArchive {
    file: RandomAccessFile,
    archive: Iar,
}

impl archive::Archive for IarArchive {
    fn extract(
        &self,
        entry: &archive::FileEntry,
    ) -> anyhow::Result<FileContents> {
        self.archive
            .file_entries
            .iter()
            .find(|e| e.file_offset == entry.file_offset)
            .map(|e| self.extract(e))
            .context("File not found")?
    }

    fn extract_all(&self, output_path: &Path) -> anyhow::Result<()> {
        self.archive.file_entries.par_iter().try_for_each(|entry| {
            let file_contents = self.extract(entry)?;
            let mut output_file_name = PathBuf::from(output_path);
            output_file_name.push(&entry.id.to_string());
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
            file_contents.write_contents(&output_file_name)?;
            Ok(())
        })
    }
}

impl IarArchive {
    fn new_root_dir(entries: &[IarFileEntry]) -> archive::Directory {
        archive::Directory::new(
            entries
                .iter()
                .map(|entry| {
                    let file_offset = entry.file_offset as u64;
                    let file_size = entry.file_size as u64;
                    archive::FileEntry {
                        file_name: entry.id.to_string(),
                        full_path: PathBuf::from(entry.id.to_string()),
                        file_offset,
                        file_size,
                    }
                })
                .collect(),
        )
    }
    fn extract(&self, entry: &IarFileEntry) -> anyhow::Result<FileContents> {
        let mut buf = BytesMut::with_capacity(entry.file_size as usize + 72);
        buf.resize(entry.file_size as usize + 72, 0);
        self.file.read_exact_at(entry.file_offset, &mut buf)?;
        Ok(FileContents {
            contents: buf.freeze(),
            type_hint: Some(ResourceMagic::Iar),
        })
    }
}

#[derive(Debug)]
struct Iar {
    header: IarHeader,
    file_entries: Vec<IarFileEntry>,
}

#[derive(Debug, Pread, Copy, Clone)]
struct IarHeader {
    major_version: u16,
    minor_version: u16,
    unk0: u32,
    some_size: u32,
    timestamp: u64,
    entry_count: u32,
    entry_count2: u32,
}

#[derive(Debug)]
struct IarFileEntry {
    version: u32,
    unk0: u32,
    decompressed_file_size: u32,
    unk1: u32,
    file_size: u32,
    unk2: u32,
    unk3: u32,
    unk4: u32,
    width: u32,
    height: u32,
    file_offset: u64,
    id: u64,
}

impl IarFileEntry {
    fn versions_to_ignore(&self) -> bool {
        match self.version & 0xFFFF {
            // This just concatenates two images into one, those images are already extracted no need to double
            0x103C | 0x101C => true,
            // TODO those modify existing image, have to refactor some stuff
            0x83C | 0x81C => true,
            _ => false,
        }
    }
}

impl<'a> ctx::TryFromCtx<'a, (u64, u64)> for IarFileEntry {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        (entry_offset, id): (u64, u64),
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let version = buf.gread_with(off, LE)?;
        let unk0 = buf.gread_with(off, LE)?;
        let decompressed_file_size = buf.gread_with(off, LE)?;
        let unk1 = buf.gread_with(off, LE)?;
        let file_size = buf.gread_with(off, LE)?;
        let unk2 = buf.gread_with(off, LE)?;
        let unk3 = buf.gread_with(off, LE)?;
        let unk4 = buf.gread_with(off, LE)?;
        let width = buf.gread_with(off, LE)?;
        let height = buf.gread_with(off, LE)?;
        let file_offset = entry_offset;
        Ok((
            IarFileEntry {
                version,
                unk0,
                decompressed_file_size,
                unk1,
                file_size,
                unk2,
                unk3,
                unk4,
                width,
                height,
                file_offset,
                id,
            },
            *off,
        ))
    }
}
