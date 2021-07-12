use std::{
    collections::HashMap, convert::TryInto, fs::File, io::Write, path::PathBuf,
};

use super::Scheme;
use crate::{
    archive::{self, Archive, FileContents, NavigableDirectory},
    error::AkaibuError,
};
use anyhow::Context;
use bytes::BytesMut;
use encoding_rs::SHIFT_JIS;
use itertools::Itertools;
use once_cell::sync::Lazy;
use positioned_io::{RandomAccessFile, ReadAt};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use scroll::{ctx, Pread, LE};

#[derive(Debug, Clone)]
pub enum PackScheme {
    KoikenOtome,
    KoikenOtomeFD,
}

static BYTE_BUF: Lazy<[u8; 256]> = Lazy::new(|| {
    let mut dest = [0u8; 256];
    dest.iter_mut().enumerate().for_each(|(i, b)| *b = i as u8);
    dest
});

const KEYS_PATH: &str = "qlie/keys.json";

static KEYS: Lazy<HashMap<String, HashMap<String, Vec<u32>>>> =
    Lazy::new(|| {
        let keys = serde_json::from_slice(
            &crate::Resources::get(KEYS_PATH)
                .expect("Could not find file: qlie/keys.json"),
        )
        .expect("Could not deserialize resource json");
        keys
    });

impl Scheme for PackScheme {
    fn extract(
        &self,
        file_path: &std::path::Path,
    ) -> anyhow::Result<(Box<dyn Archive + Sync>, NavigableDirectory)> {
        let mut buf = vec![0; 0x440];
        let metadata = std::fs::metadata(&file_path)?;
        let file = RandomAccessFile::open(file_path)?;
        file.read_exact_at(metadata.len() - 0x440, &mut buf)?;
        let header = buf.pread_with::<PackHeader>(0x440 - 0x1C, LE)?;

        if &header.magic != b"FilePackVer" && &header.version != b"3.0" {
            return Err(AkaibuError::Custom(format!(
                "Unsupported archive: {} version: {}",
                String::from_utf8_lossy(&header.magic),
                String::from_utf8_lossy(&header.version)
            ))
            .into());
        }

        let header2 = buf.pread_with::<PackHeader2>(0, LE)?;
        let header2_data = &buf[0x24..];
        log::debug!("Header: {:#?}", header);

        let decrypt_key = generate_decrypt_key(&header2_data[..0x100])?;

        let mut buf2 = vec![0; header2.hash_data_size as usize];
        file.read_exact_at(
            metadata.len() - 0x440 - header2.hash_data_size as u64,
            &mut buf2,
        )?;

        let hash_data_header = buf2.pread_with::<HashDataHeader>(0, LE)?;
        let hash_data =
            decompress(&decrypt_with_decrypt_key(&buf2[32..], 0x428)?)?;
        let entries = parse_hash_data(&hash_data, hash_data_header.iter_count)?;

        let mut entry_data = vec![
            0;
            (metadata.len() as usize
                - 0x440
                - header2.hash_data_size as usize)
                - header.entry_data_offset as usize
        ];
        file.read_exact_at(header.entry_data_offset as u64, &mut entry_data)?;
        let file_entries = parse_entry_data(&entry_data, entries)?;
        log::debug!("{:#?}", file_entries);

        let root_dir = PackArchive::new_root_dir(&file_entries);
        let navigable_dir = archive::NavigableDirectory::new(root_dir);

        let keys = KEYS
            .get(match self {
                Self::KoikenOtome => "KoikenOtome",
                Self::KoikenOtomeFD => "KoikenOtomeFD",
            })
            .context(format!("Could not find keys for {:?}", self))?;
        let key1 = keys
            .get("KEY1")
            .context("Could not find KEY1 on keys file")?
            .clone();
        let key2 = keys
            .get("KEY2")
            .context("Could not find KEY2 on keys file")?
            .clone();

        Ok((
            Box::new(PackArchive {
                file,
                header,
                file_entries,
                decrypt_key,
                key1,
                key2,
            }),
            navigable_dir,
        ))
    }

    fn get_name(&self) -> String {
        format!(
            "[QLIE PACK] {}",
            match self {
                PackScheme::KoikenOtome => "Koiken Otome",
                PackScheme::KoikenOtomeFD => "Koiken Otome ~Revive~",
            }
        )
    }

    fn get_schemes() -> Vec<Box<dyn Scheme>>
    where
        Self: Sized,
    {
        vec![Box::new(Self::KoikenOtome), Box::new(Self::KoikenOtomeFD)]
    }
}

#[derive(Debug)]
struct PackArchive {
    file: RandomAccessFile,
    header: PackHeader,
    file_entries: Vec<PackFileEntry>,
    decrypt_key: u32,
    key1: Vec<u32>,
    key2: Vec<u32>,
}

impl archive::Archive for PackArchive {
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

impl PackArchive {
    fn new_root_dir(entries: &[PackFileEntry]) -> archive::Directory {
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
    fn extract(&self, entry: &PackFileEntry) -> anyhow::Result<FileContents> {
        let mut buf = BytesMut::with_capacity(entry.file_size as usize);
        buf.resize(entry.file_size as usize, 0);

        self.file.read_exact_at(entry.file_offset, &mut buf)?;

        if entry.unk1 == 4 {
            let mut prng = Prng::init_prng(
                &entry.file_name,
                entry.file_size,
                self.decrypt_key,
                &self,
            );
            prng.decrypt(&mut buf)?;
        }
        if entry.unk0 != 0 {
            buf = BytesMut::from(&decompress(&buf)?[..]);
        }

        Ok(FileContents {
            contents: buf.freeze(),
            type_hint: None,
        })
    }
}

#[derive(Debug, Pread)]
struct PackHeader {
    magic: [u8; 11],
    version: [u8; 3],
    unk0: u16,
    unk1: u32,
    entry_data_offset: u32,
    unk3: u32,
}

#[derive(Debug, Pread)]
struct PackHeader2 {
    key: [u8; 32],
    hash_data_size: u32,
}

#[derive(Debug, Pread)]
struct HashDataHeader {
    magic: [u8; 7],
    version: [u8; 3],
    unk0: u16,
    unk1: u32,
    iter_count: u32,
    unk3: u32,
    unk4: u32,
    data_size: u32,
}

#[derive(Debug)]
struct PackEntry {
    name_size: u16,
    full_path: PathBuf,
    id: u64,
    unk0: u32,
    file_name: Vec<u8>,
}

impl<'a> ctx::TryFromCtx<'a, ()> for PackEntry {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        _ctx: (),
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let name_size = buf.gread_with::<u16>(off, LE)?;
        let file_name = buf[*off..*off + name_size as usize].to_vec();
        let full_path = PathBuf::from(
            SHIFT_JIS
                .decode(&buf[*off..*off + name_size as usize])
                .0
                .into_owned()
                .replace("\\", "/"),
        );
        *off += name_size as usize;
        let id = buf.gread_with::<u64>(off, LE)?;
        let unk0 = buf.gread_with::<u32>(off, LE)?;
        Ok((
            PackEntry {
                name_size,
                full_path,
                id,
                unk0,
                file_name,
            },
            *off,
        ))
    }
}

#[derive(Debug)]
struct PackFileEntry {
    name_size: u16,
    full_path: PathBuf,
    file_offset: u64,
    file_size: u32,
    decompressed_file_size: u32,
    unk0: u32,
    unk1: u32,
    checksum: u32,
    file_name: Vec<u8>,
}

impl<'a> ctx::TryFromCtx<'a, &'a PackEntry> for PackFileEntry {
    type Error = anyhow::Error;

    fn try_from_ctx(
        buf: &'a [u8],
        entry: &'a PackEntry,
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let name_size = buf.gread_with::<u16>(off, LE)?;
        let full_path = entry.full_path.clone();
        *off += name_size as usize;
        let file_offset = buf.gread_with::<u64>(off, LE)?;
        let file_size = buf.gread_with::<u32>(off, LE)?;
        let decompressed_file_size = buf.gread_with::<u32>(off, LE)?;
        let unk0 = buf.gread_with::<u32>(off, LE)?;
        let unk1 = buf.gread_with::<u32>(off, LE)?;
        let checksum = buf.gread_with::<u32>(off, LE)?;
        let file_name = entry.file_name.clone();
        Ok((
            PackFileEntry {
                name_size,
                full_path,
                file_offset,
                file_size,
                decompressed_file_size,
                unk0,
                unk1,
                checksum,
                file_name,
            },
            *off,
        ))
    }
}

fn generate_decrypt_key(src: &[u8]) -> anyhow::Result<u32> {
    let mut mm0 = [0u8; 8];
    let mut mm2 = [0u8; 8];
    let mm3 = [0x7, 0x3, 0x7, 0x3, 0x7, 0x3, 0x7, 0x3];
    src.chunks_exact(8)
        .try_for_each::<_, anyhow::Result<()>>(|c| {
            let mut mm1: [u8; 8] = c.try_into().expect("Chunks failed");
            paddw(&mut mm2, &mm3)?;
            pxor(&mut mm1, &mm2);
            paddw(&mut mm0, &mm1)?;
            Ok(())
        })?;
    let result =
        mm0.pread_with::<u32>(0, LE)? ^ mm0.pread_with::<u32>(4, LE)?;
    Ok(result & 0x0FFF_FFFF)
}

fn decrypt_with_decrypt_key(
    src: &[u8],
    decrypt_key: u32,
) -> anyhow::Result<Vec<u8>> {
    let mut dest = vec![0; src.len()];
    let decrypt_key =
        src.len().wrapping_add(decrypt_key as usize) as u32 ^ 0xFEC9753E;
    let mut mm7 = [0x9D, 0x5F, 0x3C, 0xA7, 0x9D, 0x5F, 0x3C, 0xA7];
    let mm6 = [0x23, 0xF5, 0x24, 0xCE, 0x23, 0xF5, 0x24, 0xCE];
    let mut mm5 = punpckldq(decrypt_key, decrypt_key);
    src.chunks_exact(8)
        .enumerate()
        .try_for_each::<_, anyhow::Result<()>>(|(i, c)| {
            paddd(&mut mm7, &mm6)?;
            pxor(&mut mm7, &mm5);
            let mut mm0: [u8; 8] = c.try_into().expect("Chunks failed");
            pxor(&mut mm0, &mm7);
            mm5 = mm0;
            dest[i * 8..i * 8 + 8].copy_from_slice(&mm0);
            Ok(())
        })?;
    Ok(dest)
}

fn decompress(src: &[u8]) -> anyhow::Result<Vec<u8>> {
    if &src[0..4] != b"1PC\xFF" {
        return Err(AkaibuError::Custom(format!(
            "Invalid decompress magic {:?}",
            &src[0..4]
        ))
        .into());
    }
    let val4 = src.pread_with::<u32>(4, LE)?;
    let dest_size = src.pread_with::<u32>(8, LE)? as usize;
    let mut dest = vec![0; dest_size];

    let index = &mut 12;
    let mut dest_index = 0;
    let mut some_buf2 = [0u8; 256];
    let mut some_buf3 = [0u8; 256];

    while *index < src.len() {
        let mut b = 0u32;
        let mut cur_buf = BYTE_BUF.clone();
        let mut byte = src.gread::<u8>(index)?;
        loop {
            if byte > 0x7F {
                b += byte as u32 - 0x7F;
                byte = 0;
            }
            if b > 0xFF {
                break;
            }
            let mut d = byte + 1;
            while d != 0 {
                cur_buf[b as usize] = src.gread::<u8>(index)?;
                if b != cur_buf[b as usize] as u32 {
                    some_buf2[b as usize] = src.gread::<u8>(index)?;
                }
                b += 1;
                d -= 1;
            }
            if b > 0xFF {
                break;
            }
            byte = src.gread(index)?;
        }

        let mut val_c = if (val4 & 1) == 1 {
            src.gread_with::<u16>(index, LE)? as u32
        } else {
            src.gread_with::<u32>(index, LE)?
        };

        let mut counter = 0;
        loop {
            if counter != 0 {
                counter -= 1;
                b = some_buf3[counter] as u32;
            } else {
                if val_c == 0 {
                    break;
                }
                val_c -= 1;
                b = src.gread::<u8>(index)? as u32;
            }
            if b == cur_buf[b as usize] as u32 {
                dest[dest_index] = b as u8;
                dest_index += 1;
            } else {
                some_buf3[counter] = some_buf2[b as usize];
                counter += 1;
                some_buf3[counter] = cur_buf[b as usize];
                counter += 1;
            }
        }
    }

    Ok(dest)
}

fn parse_hash_data(
    src: &[u8],
    iter_count: u32,
) -> anyhow::Result<Vec<PackEntry>> {
    let mut entries = Vec::new();
    let off = &mut 0;
    for _ in 0..iter_count {
        let x = src.gread_with::<u16>(off, LE)?;
        for _ in 0..x {
            entries.push(src.gread::<PackEntry>(off)?);
        }
    }
    Ok(entries)
}

fn parse_entry_data(
    src: &[u8],
    entries: Vec<PackEntry>,
) -> anyhow::Result<Vec<PackFileEntry>> {
    let mut file_entries = Vec::with_capacity(entries.len());
    let off = &mut 0;
    entries
        .iter()
        .sorted_by(|a, b| a.id.cmp(&b.id))
        .try_for_each::<_, anyhow::Result<()>>(|hash_entry| {
            file_entries
                .push(src.gread_with::<PackFileEntry>(off, hash_entry)?);
            Ok(())
        })?;
    Ok(file_entries)
}

fn punpckldq(a: u32, b: u32) -> [u8; 8] {
    let mut dest = [0; 8];
    dest[0..4].copy_from_slice(&a.to_le_bytes());
    dest[4..8].copy_from_slice(&b.to_le_bytes());
    dest
}

fn pxor(mm0: &mut [u8; 8], mm1: &[u8; 8]) {
    for i in 0..mm0.len() {
        mm0[i] ^= mm1[i];
    }
}

fn paddb(mm0: &mut [u8; 8], mm1: &[u8; 8]) {
    mm0.iter_mut()
        .zip(mm1.iter())
        .for_each(|(b1, b2)| *b1 = b1.wrapping_add(*b2));
}

fn paddw(mm0: &mut [u8; 8], mm1: &[u8; 8]) -> anyhow::Result<()> {
    for i in 0..4 {
        let v = mm0[i * 2..i * 2 + 2]
            .pread_with::<u16>(0, LE)?
            .wrapping_add(mm1[i * 2..i * 2 + 2].pread_with::<u16>(0, LE)?);
        mm0[i * 2..i * 2 + 2].copy_from_slice(&v.to_le_bytes());
    }
    Ok(())
}

fn paddd(mm0: &mut [u8; 8], mm1: &[u8; 8]) -> anyhow::Result<()> {
    for i in 0..2 {
        let v = mm0[i * 4..i * 4 + 4]
            .pread_with::<u32>(0, LE)?
            .wrapping_add(mm1[i * 4..i * 4 + 4].pread_with::<u32>(0, LE)?);
        mm0[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    Ok(())
}

fn pslld(mm0: &mut [u8; 8], x: u32) -> anyhow::Result<()> {
    mm0.chunks_exact_mut(4)
        .try_for_each::<_, anyhow::Result<()>>(|c| {
            let mut v = c.pread_with::<u32>(0, LE)?;
            v = v.wrapping_shl(x);
            c.copy_from_slice(&v.to_le_bytes());
            Ok(())
        })
}

#[derive(Debug)]
struct Prng {
    state: [u32; 0x40],
    index: usize,
    val_9d4: u32,
    val_9d8: u32,
    val_9cc: u32,
}

impl Prng {
    fn init_prng(
        file_name: &[u8],
        file_size: u32,
        decrypt_key: u32,
        archive: &PackArchive,
    ) -> Self {
        let mut d: u32 = 0x85F532;
        let mut b: u32 = 0x33F641;
        file_name.iter().enumerate().for_each(|(i, byte)| {
            d = d.wrapping_add(*byte as u32 * (i & 0xFF) as u32);
            b ^= d;
        });
        let mut a = (file_size ^ 0x8F32DC) ^ d;
        a = a.wrapping_add(d);
        a = a.wrapping_add(file_size);
        d = file_size & 0xFFFFFF;
        let c = d;
        d = d.wrapping_add(d);
        d = d.wrapping_add(d);
        d = d.wrapping_add(d);
        d = d.wrapping_sub(c);
        a = a.wrapping_add(d);
        a ^= decrypt_key;
        b = b.wrapping_add(a);
        a = b & 0xFFFFFF;
        a = a.wrapping_add(a.wrapping_mul(8));
        a ^= 0x453A;
        d = a;
        let mut state = [0; 0x40];
        state[0] = d;
        let val_9d4 = 0x9C4F88E3;
        let val_9d8 = 0xE7F70000;
        let val_9cc = 1;
        for i in 0..0x3F {
            let prev = state[i];
            let mut x = prev;
            x >>= 0x1E;
            x ^= prev;
            x = x.wrapping_mul(0x6611BC19);
            x = x.wrapping_add(i as u32 + 1);
            state[i + 1] = x
        }
        for i in 0..0x40 {
            state[i] ^= archive.key1[i];
        }
        for i in 0..0x40 {
            state[i] ^= archive.key2[i];
        }
        let index = 0;
        Prng {
            state,
            index,
            val_9d4,
            val_9d8,
            val_9cc,
        }
    }
    fn next(&mut self) -> u32 {
        self.val_9cc -= 1;
        if self.val_9cc == 0 {
            self.val_9cc = 0x40;
            self.index = 0;
            let mut index = 0;
            for _ in 0..0x40 - 0x27 {
                let mut a = self.state[index];
                let d = self.state[index + 1];
                a = Self::mod_a_d(a, d);
                let d = 0x27 + index;
                a ^= self.state[d];
                self.state[index] = a;
                index += 1;
            }
            for _ in 0..0x27 - 1 {
                let mut a = self.state[index];
                let d = self.state[index + 1];
                a = Self::mod_a_d(a, d);
                let d = index - 25;
                a ^= self.state[d];
                self.state[index] = a;
                index += 1;
            }
            let mut a = self.state[index];
            let d = self.state[0];
            a = Self::mod_a_d(a, d);
            let d = index - 25;
            a ^= self.state[d];
            self.state[index] = a;
        }
        let mut a = self.state[self.index];
        self.index += 1;
        let result = a;
        let mut d = result;
        a >>= 0xB;
        d ^= a;
        a = d;
        a = (a << 7) & 0xFFFF_FFFF;
        a &= self.val_9d4;
        d ^= a;
        a = d;
        a = (a << 0xF) & 0xFFFF_FFFF;
        a &= self.val_9d8;
        d ^= a;
        a = d;
        a >>= 0x12;
        d ^= a;
        d
    }
    fn mod_a_d(mut a: u32, d: u32) -> u32 {
        a &= 0x8000_0000;
        let mut c = 0x7FFF_FFFF;
        c &= d;
        c >>= 1;
        a |= c;
        if ((d & 0xFF) & 1) != 0 {
            a ^= 0x9908B0DF;
        }
        a
    }
    fn decrypt(&mut self, src: &mut [u8]) -> anyhow::Result<()> {
        let mut randoms_array = [0u8; 41 * 4];
        for i in 0..41 {
            randoms_array[i * 4..i * 4 + 4]
                .copy_from_slice(&self.next().to_le_bytes());
        }
        let mut mm7 = punpckldq(self.next(), self.next());
        let mut index = (self.next() & 0xF) as usize;
        index = index.wrapping_add(index);
        index = index.wrapping_add(index);
        index = index.wrapping_add(index);

        src.chunks_exact_mut(8)
            .try_for_each::<_, anyhow::Result<()>>(|c| {
                let mm6: [u8; 8] =
                    randoms_array[index..index + 8].try_into()?;
                pxor(&mut mm7, &mm6);
                paddd(&mut mm7, &mm6)?;
                let mut mm0: [u8; 8] = c[..].try_into()?;
                pxor(&mut mm0, &mm7);
                let mm1 = mm0;
                c.copy_from_slice(&mm0);
                paddb(&mut mm7, &mm1);
                pxor(&mut mm7, &mm1);
                pslld(&mut mm7, 1)?;
                paddw(&mut mm7, &mm1)?;
                index += 8;
                index &= 0x7F;

                Ok(())
            })?;
        Ok(())
    }
}
