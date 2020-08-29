use super::Scheme;
use crate::archive;
use anyhow::Context;
use bytes::{Bytes, BytesMut};
use scroll::{ctx, Pread, LE};
use std::{
    fs::File,
    io::{Read, Seek},
    path::PathBuf,
};

const PASSWORD: &[u8] = &[
    0x40, 0x21, 0x28, 0x38, 0xA6, 0x6E, 0x43, 0xA5, 0x40, 0x21, 0x28, 0x38,
    0xA6, 0x43, 0xA5, 0x64, 0x3E, 0x65, 0x24, 0x20, 0x46, 0x6E, 0x74,
];

#[derive(Debug)]
pub enum GxpScheme {
    Universal,
}

impl Scheme for GxpScheme {
    fn extract(
        &self,
        file_path: &PathBuf,
    ) -> anyhow::Result<Box<dyn archive::Archive + Sync>> {
        let mut buf = vec![0; 48];
        let mut file = File::open(file_path)?;
        file.read_exact(&mut buf)?;
        let header = buf.pread::<GxpHeader>(0)?;
        log::debug!("Header: {:#?}", header);

        let mut buf = vec![0; header.file_entries_size as usize];
        file.read_exact(&mut buf)?;
        let gxp = buf.pread_with::<Gxp>(0, header)?;
        log::debug!("Archive: {:?}", gxp);

        Ok(Box::new(GxpArchive {
            file_path: file_path.clone(),
            archive: gxp,
        }))
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

struct GxpArchive {
    file_path: PathBuf,
    archive: Gxp,
}

impl archive::Archive for GxpArchive {
    fn extract(&self, entry: &archive::FileEntry) -> anyhow::Result<Bytes> {
        self.archive
            .file_entries
            .iter()
            .find(|e| e.file_name == entry.file_name)
            .map(|e| self.extract(e))
            .context("File not found")?
    }
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
}

impl GxpArchive {
    fn extract(&self, entry: &GxpFileEntry) -> anyhow::Result<Bytes> {
        let mut file = File::open(&self.file_path)?;
        file.seek(std::io::SeekFrom::Start(
            self.archive.header.raw_file_data_offset as u64
                + entry.file_offset as u64,
        ))?;
        let mut buf = BytesMut::with_capacity(entry.file_size as usize);
        buf.resize(entry.file_size as usize, 0);
        let buf_len = buf.len();

        file.read_exact(&mut buf)?;
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
    file_name: String,
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
            let file_name = String::from_utf16(&utf16_string)?;
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
                    file_name,
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
            let file_name = String::from_utf16(&utf16_string)?;
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
                    file_name,
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
