use super::Scheme;
use crate::{
    archive::{self, FileContents},
    error::AkaibuError,
};
use anyhow::Context;
use bytes::{BufMut, Bytes, BytesMut};
use camellia_rs::{Block, CamelliaCipher};
use positioned_io::{RandomAccessFile, ReadAt};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use scroll::{ctx, Pread, LE};
use std::{collections::HashMap, fs::File, io::Write, path::PathBuf};

const KEYS_PATH: &str = "malie/keys.json";
const MAGIC: &[u8] = b"LIBP";

#[derive(Debug, Clone)]
pub enum MalieScheme {
    HaruUso,
    NatsuUso,
}

impl Scheme for MalieScheme {
    fn extract(
        &self,
        file_path: &std::path::Path,
    ) -> anyhow::Result<(
        Box<dyn crate::archive::Archive + Sync>,
        crate::archive::NavigableDirectory,
    )> {
        let camellia =
            CamelliaCipher::new(&self.get_game_key()?).map_err(|_| {
                AkaibuError::Custom("Invalid Camellia key length".to_owned())
            })?;
        let mut buf = vec![0; 16];
        let file = RandomAccessFile::open(file_path)?;
        file.read_exact_at(0, &mut buf)?;
        decrypt(&mut buf, 0, &camellia)?;

        let header = buf.pread::<MalieHeader>(0)?;
        log::debug!("Header: {:#?}", header);
        if header.magic != MAGIC {
            return Err(AkaibuError::Custom(format!(
                "Invalid magic valie for malie archive: {:X?}",
                header.magic
            ))
            .into());
        }
        let size = ((header.entry_count * 8 + header.unk2) * 4) as usize;
        let file_data_offset = ((((header.entry_count * 8 + header.unk2) * 4
            + 0x10)
            + 1023)
            >> 10) as u64;
        let file_entries_size = (header.entry_count << 5) as usize;
        let mut buf = vec![0; align_size(size)];
        file.read_exact_at(16, &mut buf)?;
        buf.chunks_mut(16)
            .enumerate()
            .try_for_each::<_, anyhow::Result<()>>(|(i, c)| {
                decrypt(c, ((i + 1) * 0x10) as u32, &camellia)?;
                Ok(())
            })?;
        buf.resize(size, 0);
        let file_offset_table: Vec<u64> = buf[file_entries_size..]
            .chunks_exact(4)
            .try_fold::<_, _, anyhow::Result<Vec<u64>>>(
                Vec::with_capacity(file_entries_size / 4),
                |mut v, c| {
                    v.push(c.pread_with::<u32>(0, LE)? as u64);
                    Ok(v)
                },
            )?;
        let mut file_entries: Vec<MalieEntry> = buf[..file_entries_size]
            .chunks_exact(32)
            .enumerate()
            .try_fold::<_, _, anyhow::Result<Vec<MalieEntry>>>(
                Vec::with_capacity(header.entry_count as usize),
                |mut v, (i, c)| {
                    v.push(c.pread_with(0, (i, &file_offset_table[..]))?);
                    Ok(v)
                },
            )?;
        let directories: Vec<(usize, String, std::ops::Range<usize>)> =
            file_entries
                .iter()
                .filter(|entry| entry.file_type == EntryType::Directory)
                .map(|entry| {
                    (
                        entry.id,
                        entry.file_name.clone(),
                        (entry.file_offset as usize
                            ..entry.file_offset as usize
                                + entry.file_size as usize),
                    )
                })
                .collect();
        file_entries = file_entries
            .into_iter()
            .filter(|entry| entry.file_type == EntryType::File)
            .map(|mut entry| {
                let mut path = get_path(entry.id, &directories);
                path.push(&entry.file_name);
                entry.full_path = path;
                entry
            })
            .collect();
        let archive = Malie {
            header,
            file_entries,
        };
        log::debug!("Archive: {:#?}", archive);

        let root_dir = MalieArchive::new_root_dir(&archive.file_entries);
        let navigable_dir = archive::NavigableDirectory::new(root_dir);
        Ok((
            Box::new(MalieArchive {
                file,
                archive,
                camellia,
                file_data_offset,
            }),
            navigable_dir,
        ))
    }

    fn get_name(&self) -> String {
        format!(
            "[MALIE] {}",
            match self {
                Self::HaruUso => "Haru Uso -Passing Memories-",
                Self::NatsuUso => "Natsu Uso -Ahead of the Reminiscence-",
            }
        )
    }

    fn get_schemes() -> Vec<Box<dyn Scheme>>
    where
        Self: Sized,
    {
        vec![Box::new(Self::HaruUso), Box::new(Self::NatsuUso)]
    }
}

impl MalieScheme {
    fn get_game_key(&self) -> anyhow::Result<Vec<u8>> {
        let keys: HashMap<String, Vec<u8>> = serde_json::from_slice(
            &crate::Resources::get(KEYS_PATH).context(format!(
                "Could not find embedded resource: {}",
                KEYS_PATH
            ))?,
        )?;
        Ok(keys
            .get(match self {
                Self::HaruUso => "HaruUso",
                Self::NatsuUso => "NatsuUso",
            })
            .context("Malie key not found")?
            .clone())
    }
}

#[derive(Debug)]
struct MalieArchive {
    file: RandomAccessFile,
    archive: Malie,
    camellia: CamelliaCipher,
    file_data_offset: u64,
}

impl archive::Archive for MalieArchive {
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
        self.archive.file_entries.par_iter().try_for_each(
            |entry| -> Result<(), anyhow::Error> {
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
            },
        )
    }
}

impl MalieArchive {
    fn new_root_dir(entries: &[MalieEntry]) -> archive::Directory {
        archive::Directory::new(
            entries
                .iter()
                .map(|entry| {
                    let file_offset = entry.file_offset as u64;
                    let file_size = entry.file_size as u64;
                    archive::FileEntry {
                        file_name: entry.file_name.clone(),
                        full_path: entry.full_path.clone(),
                        file_offset,
                        file_size,
                    }
                })
                .collect(),
        )
    }
    fn extract(&self, entry: &MalieEntry) -> anyhow::Result<FileContents> {
        let aligned = align_size(entry.file_size as usize);
        let offset =
            (entry.file_offset as usize + self.file_data_offset as usize) << 10;
        let mut buf = BytesMut::with_capacity(aligned);
        buf.resize(aligned, 0);
        self.file.read_exact_at(offset as u64, &mut buf)?;
        decrypt_file(&mut buf, offset, &self.camellia)?;
        buf.resize(entry.file_size as usize, 0);
        Ok(FileContents {
            contents: buf.freeze(),
            type_hint: None,
        })
    }
}

#[derive(Debug)]
struct Malie {
    header: MalieHeader,
    file_entries: Vec<MalieEntry>,
}

#[derive(Debug, Pread)]
struct MalieHeader {
    magic: [u8; 4],
    entry_count: u32,
    unk2: u32,
    unk3: u32,
}

#[derive(Debug)]
struct MalieEntry {
    id: usize,
    file_offset: u64,
    file_size: u32,
    file_type: EntryType,
    file_name: String,
    full_path: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
enum EntryType {
    Directory,
    File,
}

impl EntryType {
    fn new(x: u16) -> anyhow::Result<Self> {
        Ok(match x {
            0 => Self::Directory,
            1 => Self::File,
            _ => {
                return Err(AkaibuError::Custom(format!(
                    "MAILE File type not recongnized {}",
                    x
                ))
                .into())
            }
        })
    }
}

impl<'a> ctx::TryFromCtx<'a, (usize, &[u64])> for MalieEntry {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        (id, file_offset_table): (usize, &[u64]),
    ) -> Result<(Self, usize), Self::Error> {
        let file_name = String::from_utf8(Vec::from(
            buf.get(..22).context("Out of bounds read")?,
        ))?
        .trim_matches(char::from(0))
        .to_owned();
        let full_path = PathBuf::with_capacity(32);
        let off = &mut 22;
        let file_type = EntryType::new(buf.gread_with::<u16>(off, LE)?)?;
        let file_offset = match file_type {
            EntryType::Directory => buf.gread_with::<u32>(off, LE)? as u64,
            EntryType::File => *file_offset_table
                .get(buf.gread_with::<u32>(off, LE)? as usize)
                .context("Could not get file offset from table")?,
        };
        let file_size = buf.gread_with::<u32>(off, LE)?;
        Ok((
            Self {
                id,
                file_offset,
                file_size,
                file_type,
                file_name,
                full_path,
            },
            *off,
        ))
    }
}

fn rotate_buffer(buf: &[u8], mut n: u32) -> anyhow::Result<Bytes> {
    let mut result = BytesMut::with_capacity(16);
    n >>= 4;
    n &= 0xF;
    n += 0x10;
    buf.chunks_exact(4)
        .enumerate()
        .try_for_each::<_, anyhow::Result<()>>(|(i, c)| {
            let v = c.pread_with::<u32>(0, LE)?;
            result.put_u32_le(if i % 2 == 0 {
                v.rotate_left(n)
            } else {
                v.rotate_right(n)
            });
            Ok(())
        })?;
    Ok(result.freeze())
}

fn align_size(size: usize) -> usize {
    if size % 0x10 == 0 {
        size
    } else {
        size + (0x10 - size % 0x10)
    }
}

fn decrypt(
    buf: &mut [u8],
    n: u32,
    camellia: &CamelliaCipher,
) -> anyhow::Result<()> {
    let rotated = rotate_buffer(buf, n)?;
    let mut block = Block::default();
    block.bytes.copy_from_slice(&rotated);
    camellia.decrypt(&mut block);
    buf.iter_mut().enumerate().for_each(|(i, b)| {
        *b = block.bytes[i];
    });
    Ok(())
}

fn get_path(
    id: usize,
    directories: &[(usize, String, std::ops::Range<usize>)],
) -> PathBuf {
    let mut cur = id;
    let mut path = String::new();
    while cur != 0 {
        for (k, name, r) in directories.iter().rev() {
            if r.contains(&cur) {
                path = format!("{}/{}", name, path);
                cur = *k;
                break;
            }
        }
    }
    PathBuf::from(path.trim_start_matches('/'))
}

fn decrypt_file(
    buf: &mut [u8],
    offset: usize,
    camellia: &CamelliaCipher,
) -> anyhow::Result<()> {
    buf.chunks_mut(16).enumerate().try_for_each(|(i, chunk)| {
        decrypt(chunk, offset as u32 + i as u32 * 16, camellia)?;
        Ok(())
    })
}
