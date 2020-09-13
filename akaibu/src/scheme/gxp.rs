use super::Scheme;
use crate::archive;
use anyhow::Context;
use bytes::{Bytes, BytesMut};
use positioned_io::{RandomAccessFile, ReadAt};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use scroll::{ctx, Pread, LE};
use std::{fs::File, io::Write, path::PathBuf};

const PASSWORD: &[u8] = &[
    0x40, 0x21, 0x28, 0x38, 0xA6, 0x6E, 0x43, 0xA5, 0x40, 0x21, 0x28, 0x38,
    0xA6, 0x43, 0xA5, 0x64, 0x3E, 0x65, 0x24, 0x20, 0x46, 0x6E, 0x74,
];

#[derive(Debug, Clone)]
pub enum GxpScheme {
    Universal,
}

impl Scheme for GxpScheme {
    fn extract(
        &self,
        file_path: &PathBuf,
    ) -> anyhow::Result<(
        Box<dyn archive::Archive + Sync>,
        archive::NavigableDirectory,
    )> {
        let mut buf = vec![0; 48];
        let file = RandomAccessFile::open(file_path)?;
        file.read_exact_at(0, &mut buf)?;
        let header = buf.pread::<GxpHeader>(0)?;
        log::debug!("Header: {:#?}", header);

        buf.resize(header.file_entries_size as usize, 0);
        file.read_exact_at(48, &mut buf)?;
        let archive = buf.pread_with::<Gxp>(0, header)?;
        log::debug!("Archive: {:?}", archive);

        let root_dir = GxpArchive::new_root_dir(&archive.file_entries);
        let navigable_dir = archive::NavigableDirectory::new(root_dir);
        Ok((Box::new(GxpArchive { file, archive }), navigable_dir))
    }
    fn get_name(&self) -> &str {
        "GXP"
    }
    fn get_schemes() -> Vec<Box<dyn Scheme>>
    where
        Self: Sized,
    {
        vec![Box::new(Self::Universal)]
    }
}

#[derive(Debug)]
struct GxpArchive {
    file: RandomAccessFile,
    archive: Gxp,
}

impl archive::Archive for GxpArchive {
    fn extract(&self, entry: &archive::FileEntry) -> anyhow::Result<Bytes> {
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

impl GxpArchive {
    fn new_root_dir(entries: &[GxpFileEntry]) -> archive::Directory {
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
    fn extract(&self, entry: &GxpFileEntry) -> anyhow::Result<Bytes> {
        let mut buf = BytesMut::with_capacity(entry.file_size as usize);
        buf.resize(entry.file_size as usize, 0);
        let buf_len = buf.len();

        self.file.read_exact_at(
            self.archive.header.raw_file_data_offset as u64
                + entry.file_offset as u64,
            &mut buf,
        )?;
        xor_data_with_password(&mut buf, buf_len, 0);
        Ok(buf.freeze())
    }
}

#[derive(Debug)]
struct Gxp {
    header: GxpHeader,
    file_entries: Vec<GxpFileEntry>,
}

impl<'a> ctx::TryFromCtx<'a, GxpHeader> for Gxp {
    type Error = anyhow::Error;
    fn try_from_ctx(
        buf: &'a [u8],
        header: GxpHeader,
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let mut file_entries =
            Vec::with_capacity(header.file_entries_count as usize);
        for _ in 0..header.file_entries_count {
            file_entries.push(buf.gread_with::<GxpFileEntry>(off, &header)?);
        }
        Ok((
            Gxp {
                header,
                file_entries,
            },
            *off,
        ))
    }
}

#[derive(Debug, Pread, Copy, Clone)]
struct GxpHeader {
    magic: u32,
    unk1: u32,
    unk2: u32,
    unk3: u32,
    unk4: u32,
    unk5: u32,
    file_entries_count: u32,
    file_entries_size: u32,
    raw_file_data_size: u32,
    unk6: u32,
    raw_file_data_offset: u32,
    unk7: u32,
}

#[derive(Debug)]
struct GxpFileEntry {
    entry_size: u32,
    file_size: u32,
    unk1: u32,
    file_name_utf16_len: u32,
    unk2: u32,
    unk3: u32,
    file_offset: u32,
    unk4: u32,
    full_path: PathBuf,
}

impl<'a> ctx::TryFromCtx<'a, &GxpHeader> for GxpFileEntry {
    type Error = anyhow::Error;
    fn try_from_ctx(
        buf: &'a [u8],
        header: &GxpHeader,
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        if header.unk5 != 0 && header.file_entries_count != 0 {
            let mut entry_data = buf[0..4].to_vec();
            xor_data_with_password(&mut entry_data, 4, 0);
            let entry_size = entry_data.gread_with::<u32>(off, LE)?;
            let mut entry_data = buf[0..entry_size as usize].to_vec();
            xor_data_with_password(&mut entry_data, entry_size as usize - 4, 4);

            let file_size = entry_data.gread_with::<u32>(off, LE)?;
            let unk1 = entry_data.gread_with::<u32>(off, LE)?;
            let file_name_utf16_len = entry_data.gread_with::<u32>(off, LE)?;
            let unk2 = entry_data.gread_with::<u32>(off, LE)?;
            let unk3 = entry_data.gread_with::<u32>(off, LE)?;
            let file_offset = entry_data.gread_with::<u32>(off, LE)?;
            let unk4 = entry_data.gread_with::<u32>(off, LE)?;
            let utf16_string: Vec<u16> = entry_data
                [*off..*off + entry_size as usize - 0x20]
                .chunks(2)
                .map(|c| c[0] as u16 + ((c[1] as u16) << 8))
                .filter(|v| *v != 0)
                .collect();
            let full_path = PathBuf::from(String::from_utf16(&utf16_string)?);
            Ok((
                GxpFileEntry {
                    entry_size,
                    file_size,
                    unk1,
                    file_name_utf16_len,
                    unk2,
                    unk3,
                    file_offset,
                    unk4,
                    full_path,
                },
                entry_size as usize,
            ))
        } else {
            let entry_size = buf.gread_with::<u32>(off, LE)?;
            let file_size = buf.gread_with::<u32>(off, LE)?;
            let unk1 = buf.gread_with::<u32>(off, LE)?;
            let file_name_utf16_len = buf.gread_with::<u32>(off, LE)?;
            let unk2 = buf.gread_with::<u32>(off, LE)?;
            let unk3 = buf.gread_with::<u32>(off, LE)?;
            let file_offset = buf.gread_with::<u32>(off, LE)?;
            let unk4 = buf.gread_with::<u32>(off, LE)?;
            let utf16_string: Vec<u16> = buf
                [*off..*off + file_name_utf16_len as usize * 2]
                .chunks(2)
                .map(|c| c[0] as u16 + ((c[1] as u16) << 8))
                .filter(|v| *v != 0)
                .collect();
            let full_path = PathBuf::from(String::from_utf16(&utf16_string)?);
            Ok((
                GxpFileEntry {
                    entry_size,
                    file_size,
                    unk1,
                    file_name_utf16_len,
                    unk2,
                    unk3,
                    file_offset,
                    unk4,
                    full_path,
                },
                entry_size as usize,
            ))
        }
    }
}

fn xor_data_with_password(data: &mut [u8], size: usize, offset: usize) {
    for i in 0..size {
        let mut al = (offset & 0xFF) as u8;
        al += (i & 0xFF) as u8;
        al ^= PASSWORD[(i + offset) % PASSWORD.len()];
        data[offset + i] ^= al;
    }
}
