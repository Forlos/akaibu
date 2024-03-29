use super::Scheme;
use crate::{
    archive::{self, FileContents},
    error::AkaibuError,
};
use anyhow::Context;
use bytes::BytesMut;
use encoding_rs::SHIFT_JIS;
use itertools::Itertools;
use positioned_io::{RandomAccessFile, ReadAt};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use scroll::{ctx, Pread, LE};
use std::{
    convert::TryInto,
    fs::File,
    io::Write,
    path::{self, Path, PathBuf},
};

const PARAMS_KEY_MARKER: &[u8] = &[
    0xa6, 0x30, 0xa3, 0x30, 0xf3, 0x30, 0xc9, 0x30, 0xa6, 0x30, 0xcc, 0x80,
    0x6f, 0x66, 0x72, 0x82, 0x06, 0x00, 0x0f, 0x90, 0x4e, 0x90, 0x87, 0x73,
    0x04, 0x00, 0x36, 0x00, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00,
];

#[derive(Debug, Clone)]
pub enum Link6Scheme {
    Universal,
}

impl Scheme for Link6Scheme {
    fn extract(
        &self,
        file_path: &path::Path,
    ) -> anyhow::Result<(
        Box<dyn crate::archive::Archive>,
        crate::archive::NavigableDirectory,
    )> {
        let mut buf = vec![0; 8 + 256];
        let file = RandomAccessFile::open(file_path)?;
        file.read_exact_at(0, &mut buf)?;
        let header = buf.pread::<Link6Header>(0)?;
        log::debug!("Header: {:#?}", header);

        let mut file_entries = Vec::new();

        let mut cur_file_offset = 8 + header.name_size as u64;
        let mut entry_size_buf = vec![0; 4];
        file.read_exact_at(cur_file_offset, &mut entry_size_buf)?;
        let mut entry_size = entry_size_buf.pread::<u32>(0)? as usize;

        while entry_size != 0 {
            let mut buf = vec![0; entry_size];
            file.read_exact_at(cur_file_offset, &mut buf)?;
            let entry = buf.pread_with(0, cur_file_offset)?;
            log::debug!("{:?}", entry);
            file_entries.push(entry);

            cur_file_offset += entry_size as u64;

            file.read_exact_at(cur_file_offset, &mut entry_size_buf)?;
            entry_size = entry_size_buf.pread::<u32>(0)? as usize;
        }

        let root_dir = Link6Archive::new_root_dir(&file_entries);
        let navigable_dir = archive::NavigableDirectory::new(root_dir);

        let is_cg = file_path
            .file_name()
            .context("Could not get file name")?
            .to_str()
            .context("Invalid string encoding")?
            .contains("cg");

        let key = if is_cg {
            let mut params_file_path = PathBuf::from(file_path);
            params_file_path.set_file_name("params.dat");
            let metadata = std::fs::metadata(&params_file_path)?;

            let mut params_buf = vec![0; metadata.len() as usize];
            let params_file = RandomAccessFile::open(params_file_path)?;
            params_file.read_exact_at(0, &mut params_buf)?;

            Some(extract_key_from_params(&params_buf)?)
        } else {
            None
        };

        Ok((
            Box::new(Link6Archive {
                file,
                file_entries,
                key,
            }),
            navigable_dir,
        ))
    }

    fn get_name(&self) -> String {
        format!(
            "[LINK6] {}",
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
struct Link6Archive {
    file: RandomAccessFile,
    file_entries: Vec<Link6FileEntry>,
    key: Option<Vec<u8>>,
}

impl archive::Archive for Link6Archive {
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

impl Link6Archive {
    fn new_root_dir(entries: &[Link6FileEntry]) -> archive::Directory {
        archive::Directory::new(
            entries
                .iter()
                .map(|entry| {
                    let file_offset = entry.file_offset;
                    let file_size = entry.file_size;
                    archive::FileEntry {
                        file_name: entry
                            .full_path
                            .to_str()
                            .expect("Not valid UTF-8")
                            .to_string(),
                        full_path: entry.full_path.clone(),
                        file_offset,
                        file_size: file_size as u64,
                    }
                })
                .collect(),
        )
    }
    fn extract(&self, entry: &Link6FileEntry) -> anyhow::Result<FileContents> {
        let mut buf = BytesMut::with_capacity(entry.file_size);
        buf.resize(entry.file_size as usize, 0);

        self.file.read_exact_at(entry.file_offset, &mut buf)?;
        if &buf[..2] == b"BM" {
            if let Some(key) = &self.key {
                let pixels_index = buf[10..].pread_with::<u32>(0, LE)? as usize;
                buf[pixels_index..]
                    .iter_mut()
                    .zip(key.iter())
                    .for_each(|(b, k)| *b ^= k);
            }
        }

        Ok(FileContents {
            contents: buf.freeze(),
            type_hint: None,
        })
    }
}

#[derive(Debug)]
struct Link6Header {
    magic: [u8; 7],
    name_size: usize,
    name: String,
}

impl<'a> ctx::TryFromCtx<'a, ()> for Link6Header {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        _ctx: (),
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 7;
        let magic = buf[0..7].try_into()?;
        let name_size = buf.gread::<u8>(off)? as usize;
        let name = SHIFT_JIS.decode(&buf[*off..*off + name_size]).0.to_string();
        *off += name_size;
        Ok((
            Self {
                magic,
                name_size,
                name,
            },
            *off,
        ))
    }
}

#[derive(Debug)]
struct Link6FileEntry {
    file_size: usize,
    file_offset: u64,
    full_path: PathBuf,
}

impl<'a> ctx::TryFromCtx<'a, u64> for Link6FileEntry {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        cur_file_offset: u64,
    ) -> Result<(Self, usize), Self::Error> {
        let entry_size = buf.pread_with::<u32>(0, LE)? as usize;
        let name_size = buf.pread_with::<u16>(13, LE)? as usize;

        let full_path = PathBuf::from(String::from_utf16(
            &buf[15..15 + name_size]
                .iter()
                .tuples()
                .map(|(x1, x2)| *x1 as u16 + ((*x2 as u16) << 8))
                .collect::<Vec<u16>>(),
        )?);
        let file_size = entry_size - name_size - 15;
        let file_offset = cur_file_offset + 15 + name_size as u64;
        Ok((
            Self {
                file_size,
                file_offset,
                full_path,
            },
            entry_size,
        ))
    }
}

fn extract_key_from_params(buf: &[u8]) -> anyhow::Result<Vec<u8>> {
    let (params_key_index, _) = buf
        .windows(PARAMS_KEY_MARKER.len())
        .enumerate()
        .find(|(_, w)| w == &PARAMS_KEY_MARKER)
        .context("Could not find key in params.dat")?;

    let version = buf[params_key_index + PARAMS_KEY_MARKER.len()];

    // let version = &buf[..0x11];
    Ok(match version {
        // b"[SCR-PARAMS]v05.4" | b"[SCR-PARAMS]v05.5" | b"[SCR-PARAMS]v05.6" => {
        0 => {
            let key_size = buf[params_key_index + PARAMS_KEY_MARKER.len() + 4..]
                .pread_with::<u32>(0, LE)? as usize;
            buf[params_key_index + PARAMS_KEY_MARKER.len() + 8
                ..params_key_index
                    + PARAMS_KEY_MARKER.len()
                    + 8
                    + key_size as usize]
                .to_vec()
        }
        // b"[SCR-PARAMS]v05.7" => {
        6 => {
            let key_size = buf
                [params_key_index + PARAMS_KEY_MARKER.len() + 0x84..]
                .pread_with::<u32>(0, LE)? as usize;
            buf[params_key_index + PARAMS_KEY_MARKER.len() + 0x88
                ..params_key_index + PARAMS_KEY_MARKER.len() + 0x88 + key_size]
                .to_vec()
        }
        _ => {
            let version = &buf[..0x11];
            return Err(AkaibuError::Custom(format!(
                "Unsupported version of params.dat file {:?}",
                String::from_utf8_lossy(version)
            ))
            .into());
        }
    })
}
