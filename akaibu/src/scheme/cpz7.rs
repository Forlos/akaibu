use super::Scheme;
use crate::{archive, util::md5};
use anyhow::Context;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use encoding_rs::SHIFT_JIS;
use positioned_io::{RandomAccessFile, ReadAt};
use scroll::{ctx, Pread, LE};
use std::{
    collections::HashMap, convert::TryInto, fs::File, io::Write, path::PathBuf,
};

/// Used to decrypt header fields
const HEADER_KEYS: [u32; 12] = [
    0xFE3A53DA, 0x37F298E8, 0x7A6F3A2D, 0x43DE7C1A, 0xCC65F416, 0xD016A93D,
    0x97A3BA9B, 0xAE7D39B7, 0xFB73A956, 0x37ACF832, 0xA7B09C72, 0x65EF99F3,
];

/// Used to decrypt files
const PASSWORD: &[u8] = &[
    137, 240, 144, 205, 130, 183, 130, 233, 136, 171, 130, 162, 142, 113, 130,
    205, 131, 138, 131, 82, 130, 170, 130, 168, 142, 100, 146, 117, 130, 171,
    130, 181, 130, 191, 130, 225, 130, 162, 130, 220, 130, 183, 129, 66, 142,
    244, 130, 237, 130, 234, 130, 191, 130, 225, 130, 162, 130, 220, 130, 183,
    130, 230, 129, 96, 129, 65, 130, 198, 130, 162, 130, 164, 130, 169, 130,
    224, 130, 164, 142, 244, 130, 193, 130, 191, 130, 225, 130, 162, 130, 220,
    130, 181, 130, 189, 129, 244,
];

#[derive(Debug, Clone)]
pub enum Cpz7Scheme {
    AoiTori,
    Realive,
    SeishunFragile,
}

impl Scheme for Cpz7Scheme {
    fn extract(
        &self,
        file_path: &PathBuf,
    ) -> anyhow::Result<(
        Box<dyn archive::Archive + Sync>,
        archive::NavigableDirectory,
    )> {
        let mut buf = vec![0; 68];
        let file = RandomAccessFile::open(file_path)?;
        file.read_exact_at(4, &mut buf)?;
        let cpz_header = buf.pread::<Cpz7Header>(0)?;

        let mut buf = vec![
            0;
            cpz_header.archive_data_size as usize
                + cpz_header.file_data_size as usize
                + cpz_header.encryption_data_size as usize
        ];
        file.read_exact_at(72, &mut buf)?;
        let all_game_keys = self.get_game_keys()?;
        let game_keys = *all_game_keys
            .get(
                file_path
                    .file_name()
                    .context("Could not get file name")?
                    .to_str()
                    .context("Could not parse OsStr to str")?,
            )
            .unwrap_or(&[0, 0, 0, 0]);
        let archive = buf.pread_with::<Cpz7>(0, (cpz_header, &game_keys))?;
        log::debug!("Archive: {:#?}", archive.file_data.values());

        let root_dir = Cpz7Archive::new_root_dir(&archive);
        let navigable_dir = archive::NavigableDirectory::new(root_dir);
        Ok((
            Box::new(Cpz7Archive {
                file,
                game_keys,
                archive,
            }),
            navigable_dir,
        ))
    }
    fn get_name(&self) -> &str {
        match self {
            Self::AoiTori => "Aoi Tori",
            Self::Realive => "Realive",
            Self::SeishunFragile => "Seishun Fragile",
        }
    }
    fn get_schemes() -> Vec<Box<dyn Scheme>>
    where
        Self: Sized,
    {
        vec![
            Box::new(Cpz7Scheme::AoiTori),
            Box::new(Cpz7Scheme::Realive),
            Box::new(Cpz7Scheme::SeishunFragile),
        ]
    }
}

impl Cpz7Scheme {
    fn get_game_keys(&self) -> anyhow::Result<HashMap<String, [u32; 4]>> {
        Ok(match self {
            Cpz7Scheme::AoiTori => serde_json::from_slice(
                &crate::Resources::get("cpz7/aoitori.json").context(
                    format!("Could not find file: {}", "cpz7/aoitori.json"),
                )?,
            )?,
            Cpz7Scheme::Realive => serde_json::from_slice(
                &crate::Resources::get("cpz7/realive.json").context(
                    format!("Could not find file: {}", "cpz7/realive.json"),
                )?,
            )?,
            Cpz7Scheme::SeishunFragile => serde_json::from_slice(
                &crate::Resources::get("cpz7/seishun.json").context(
                    format!("Could not find file: {}", "cpz7/seishun.json"),
                )?,
            )?,
        })
    }
}

#[derive(Debug)]
struct Cpz7Archive {
    file: RandomAccessFile,
    game_keys: [u32; 4],
    archive: Cpz7,
}

impl archive::Archive for Cpz7Archive {
    fn extract(&self, entry: &archive::FileEntry) -> anyhow::Result<Bytes> {
        self.archive
            .file_data
            .values()
            .flatten()
            .find(|e| e.full_path == entry.full_path)
            .map(|e| self.extract(e))
            .context("File not found")?
    }

    fn extract_all(&self, output_path: &PathBuf) -> anyhow::Result<()> {
        // TODO parallelize that
        self.archive
            .file_data
            .values()
            .flatten()
            .try_for_each(|entry| {
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

impl Cpz7Archive {
    fn new_root_dir(archive: &Cpz7) -> archive::Directory {
        archive::Directory::new(
            archive
                .file_data
                .values()
                .flatten()
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
    fn extract(&self, entry: &FileEntry) -> anyhow::Result<Bytes> {
        let mut contents = vec![0; entry.file_size as usize];
        let raw_file_data_off = self.archive.header.archive_data_size
            + self.archive.header.file_data_size
            + self.archive.header.encryption_data_size
            + 0x48;
        self.file.read_exact_at(
            raw_file_data_off as u64 + entry.file_offset as u64,
            &mut contents,
        )?;
        let file_key = get_file_key(
            &entry,
            entry.archive_file_decrypt_key,
            &self.archive.header,
            self.game_keys[2],
            self.game_keys[3],
        );
        decrypt_file(
            &contents,
            entry.file_size as usize,
            &self.archive.md5_cpz7,
            file_key,
            &self.archive.files_decrypt_table,
            &PASSWORD,
        )
    }
}

#[derive(Debug)]
struct Cpz7 {
    header: Cpz7Header,
    file_data: HashMap<ArchiveDataEntry, Vec<FileEntry>>,
    files_decrypt_table: Bytes,
    md5_cpz7: [u8; 16],
    encryption_data: EncryptionData,
}

impl<'a> ctx::TryFromCtx<'a, (Cpz7Header, &[u32; 4])> for Cpz7 {
    type Error = anyhow::Error;
    fn try_from_ctx(
        buf: &'a [u8],
        (header, game_keys): (Cpz7Header, &[u32; 4]),
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let encryption_data = buf.pread_with::<EncryptionData>(
            *off + header.archive_data_size as usize
                + header.file_data_size as usize,
            header.encryption_data_size as usize,
        )?;
        let mut raw_data = BytesMut::from(
            &buf[*off..*off
                + header.archive_data_size as usize
                + header.file_data_size as usize],
        );
        encryption_data.decrypt_buf(&mut raw_data);
        raw_data = decrypt_with_password(
            &raw_data,
            raw_data.len(),
            PASSWORD,
            header.archive_data_key ^ 0x3795B39A,
        )?;
        let md5_cpz7 = md5_cpz7(&header.cpz7_md5)?;
        let archive_data_decrypt_table = init_decrypt_table(
            header.archive_data_key,
            md5_cpz7.pread_with::<u32>(4, LE)?,
        );
        decrypt_with_decrypt_table(
            &archive_data_decrypt_table,
            &mut raw_data,
            header.archive_data_size as usize,
            0x3A,
        );
        let decrypt_buf = get_decrypt_buf(&md5_cpz7, header.archive_data_key);
        let raw_archive_data = decrypt_archive_data(
            &decrypt_buf,
            &raw_data[..header.archive_data_size as usize],
            game_keys[0],
        )?;
        let mut archive_data: Vec<ArchiveDataEntry> =
            Vec::with_capacity(header.archive_data_entry_count as usize);
        let off = &mut 0;
        for _ in 0..header.archive_data_entry_count {
            archive_data.push(raw_archive_data.gread_with(off, LE)?);
        }
        let file_data_decrypt_table = init_decrypt_table(
            header.archive_data_key,
            md5_cpz7.pread_with(8, LE)?,
        );
        let raw_file_data = decrypt_file_data(
            &archive_data,
            &mut raw_data[header.archive_data_size as usize
                ..header.archive_data_size as usize
                    + header.file_data_size as usize],
            &file_data_decrypt_table,
            &md5_cpz7,
            game_keys[1],
        )?;
        let files_decrypt_table = init_decrypt_table(
            md5_cpz7.pread_with(12, LE)?,
            header.archive_data_key,
        );
        let mut file_data = HashMap::new();
        let off = &mut 0;
        for archive in archive_data {
            let mut file_entries =
                Vec::with_capacity(archive.file_count as usize);
            for _ in 0..archive.file_count {
                file_entries.push(raw_file_data.gread_with(off, &archive)?);
            }
            file_data.insert(archive, file_entries);
        }
        Ok((
            Cpz7 {
                header,
                file_data,
                files_decrypt_table,
                md5_cpz7,
                encryption_data,
            },
            0,
        ))
    }
}

#[derive(Debug, Copy, Clone)]
struct Cpz7Header {
    archive_data_entry_count: u32,
    archive_data_size: u32,
    file_data_size: u32,
    raw_data_md5: [u8; 16],
    cpz7_md5: [u8; 16],
    archive_data_key: u32,
    unk1: u32,
    file_decrypt_key: u32,
    unk2: u32,
    encryption_data_size: u32,
    header_checksum: u32,
}

impl<'a> ctx::TryFromCtx<'a, scroll::Endian> for Cpz7Header {
    type Error = anyhow::Error;
    fn try_from_ctx(
        buf: &'a [u8],
        _: scroll::Endian,
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let archive_data_entry_count =
            buf.gread_with::<u32>(off, LE)? ^ HEADER_KEYS[0];
        let archive_data_size =
            buf.gread_with::<u32>(off, LE)? ^ HEADER_KEYS[1];
        let file_data_size = buf.gread_with::<u32>(off, LE)? ^ HEADER_KEYS[2];
        let raw_data_md5 = buf[*off..*off + 16].try_into()?;
        *off += 16;
        let mut cpz7_md5: [u8; 16] = buf[*off..*off + 16].try_into()?;
        cpz7_md5.chunks_mut(4).enumerate().for_each(|(i, c)| {
            c[0] ^= HEADER_KEYS[i + 3] as u8;
            c[1] ^= (HEADER_KEYS[i + 3] >> 8) as u8;
            c[2] ^= (HEADER_KEYS[i + 3] >> 16) as u8;
            c[3] ^= (HEADER_KEYS[i + 3] >> 24) as u8;
        });
        *off += 16;
        let archive_data_key = buf.gread_with::<u32>(off, LE)? ^ HEADER_KEYS[7];
        let unk1 = buf.gread_with::<u32>(off, LE)? ^ HEADER_KEYS[8];
        let file_decrypt_key = buf.gread_with::<u32>(off, LE)? ^ HEADER_KEYS[9];
        let unk2 = buf.gread_with::<u32>(off, LE)? ^ HEADER_KEYS[10];
        let encryption_data_size =
            buf.gread_with::<u32>(off, LE)? ^ HEADER_KEYS[11];
        let header_checksum = buf.gread_with(off, LE)?;
        Ok((
            Cpz7Header {
                archive_data_entry_count,
                archive_data_size,
                file_data_size,
                raw_data_md5,
                cpz7_md5,
                archive_data_key,
                unk1,
                file_decrypt_key,
                unk2,
                encryption_data_size,
                header_checksum,
            },
            68,
        ))
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct ArchiveDataEntry {
    entry_size: u32,
    file_count: u32,
    offset: u32,
    file_decrypt_key: u32,
    name: String,
}

impl<'a> ctx::TryFromCtx<'a, scroll::Endian> for ArchiveDataEntry {
    type Error = anyhow::Error;
    fn try_from_ctx(
        buf: &'a [u8],
        _: scroll::Endian,
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let entry_size = buf.gread_with(off, LE)?;
        let file_count = buf.gread_with(off, LE)?;
        let offset = buf.gread_with(off, LE)?;
        let file_decrypt_key = buf.gread_with(off, LE)?;
        let name = SHIFT_JIS
            .decode(&buf[*off..*off + entry_size as usize - 0x10])
            .0
            .to_string()
            .trim_matches('\0')
            .to_string();
        Ok((
            ArchiveDataEntry {
                entry_size,
                file_count,
                offset,
                file_decrypt_key,
                name,
            },
            entry_size as usize,
        ))
    }
}

#[derive(Debug)]
struct FileEntry {
    entry_size: u32,
    file_offset: u32,
    unk1: u32,
    file_size: u32,
    unk2: u32,
    unk3: u32,
    file_decrypt_key: u32,
    archive_file_decrypt_key: u32,
    full_path: PathBuf,
}

impl<'a> ctx::TryFromCtx<'a, &ArchiveDataEntry> for FileEntry {
    type Error = anyhow::Error;
    fn try_from_ctx(
        buf: &'a [u8],
        archive: &ArchiveDataEntry,
    ) -> Result<(Self, usize), Self::Error> {
        let off = &mut 0;
        let entry_size = buf.gread_with(off, LE)?;
        let file_offset = buf.gread_with(off, LE)?;
        let unk1 = buf.gread_with(off, LE)?;
        let file_size = buf.gread_with(off, LE)?;
        let unk2 = buf.gread_with(off, LE)?;
        let unk3 = buf.gread_with(off, LE)?;
        let file_decrypt_key = buf.gread_with(off, LE)?;
        let archive_file_decrypt_key = archive.file_decrypt_key;
        let full_path = PathBuf::from(format!(
            "{}/{}",
            archive.name,
            SHIFT_JIS
                .decode(&buf[*off..*off + entry_size as usize - 0x1C])
                .0
                .to_string()
                .trim_matches('\0')
        ));
        Ok((
            FileEntry {
                entry_size,
                file_offset,
                unk1,
                file_size,
                unk2,
                unk3,
                file_decrypt_key,
                archive_file_decrypt_key,
                full_path,
            },
            entry_size as usize,
        ))
    }
}

#[derive(Debug)]
struct EncryptionData {
    md5_checksum: [u8; 16],
    data_size: u32,
    key: u32,
    data: BytesMut,
}

impl<'a> ctx::TryFromCtx<'a, usize> for EncryptionData {
    type Error = anyhow::Error;
    #[inline]
    fn try_from_ctx(
        buf: &'a [u8],
        size: usize,
    ) -> Result<(Self, usize), Self::Error> {
        let md5_checksum = buf[0..16].try_into()?;
        let off = &mut 16;
        let data_size = buf.gread_with::<u32>(off, LE)?;
        let key = buf.gread_with::<u32>(off, LE)?;
        let data = BytesMut::from(&buf[*off..size]);
        let encryption_data = EncryptionData {
            md5_checksum,
            data_size,
            key,
            data,
        };
        Ok((encryption_data.decrypt()?, 24 + data_size as usize))
    }
}

impl EncryptionData {
    fn decrypt(mut self) -> anyhow::Result<Self> {
        let mut dest = BytesMut::with_capacity(self.data_size as usize);
        dest.resize(self.data_size as usize, 0);
        let num = &mut 0x100;
        let xor_key = self.key;
        self.data.chunks_mut(4).for_each(|c| {
            c[0] ^= xor_key as u8;
            c[1] ^= (xor_key >> 8) as u8;
            c[2] ^= (xor_key >> 16) as u8;
            c[3] ^= (xor_key >> 24) as u8;
        });

        let off = &mut 0;
        let mut data1 = vec![0; 512];
        let mut data2 = vec![0; 512];
        let y = &mut 0;
        let z = &mut 0;
        let result = EncryptionData::recursive_decrypt(
            &mut self.data,
            &mut data1,
            &mut data2,
            y,
            z,
            num,
            off,
        )?;
        for i in 0..self.data_size as usize {
            let mut inner_result = result;
            if inner_result >= 0x100 {
                loop {
                    if *y == 0 {
                        *z = self.data.gread_with::<u32>(off, LE)?;
                        *y = 32;
                    }
                    *y -= 1;
                    let temp = *z;
                    *z >>= 1;
                    if temp & 1 == 0 {
                        inner_result = data1[inner_result as usize] as u32;
                    } else {
                        inner_result = data2[inner_result as usize] as u32;
                    }
                    if inner_result < 0x100 {
                        break;
                    }
                }
                dest[i] = inner_result as u8;
            }
        }
        self.data = dest;
        Ok(self)
    }
    fn recursive_decrypt(
        data: &mut [u8],
        data1: &mut [u32],
        data2: &mut [u32],
        y: &mut u32,
        z: &mut u32,
        num: &mut u32,
        off: &mut usize,
    ) -> anyhow::Result<u32> {
        if *y == 0 {
            *z = data.gread_with::<u32>(off, LE)?;
            *y = 32;
        }
        *y -= 1;
        let temp = *z;
        *z >>= 1;
        Ok(if temp & 1 == 0 {
            EncryptionData::zero_transform(8, data, y, z, off)?
        } else {
            let temp = *num;
            *num += 1;
            data1[temp as usize] = EncryptionData::recursive_decrypt(
                data, data1, data2, y, z, num, off,
            )?;
            data2[temp as usize] = EncryptionData::recursive_decrypt(
                data, data1, data2, y, z, num, off,
            )?;
            temp
        })
    }
    fn zero_transform(
        mut n: u32,
        data: &mut [u8],
        y: &mut u32,
        z: &mut u32,
        off: &mut usize,
    ) -> anyhow::Result<u32> {
        let mut result = 0;
        if n > *y {
            if n == 0 {
                return Ok(result);
            }
            loop {
                n -= 1;
                if *y == 0 {
                    *z = data.gread_with(off, LE)?;
                    *y = 32;
                }
                *y -= 1;
                result = (*z & 1) + result * 2;
                *z >>= 1;

                if n == 0 {
                    break;
                }
            }
        } else {
            if n == 0 {
                return Ok(result);
            }
            let mut temp = *z;
            let mut temp2 = *y;
            loop {
                result = (temp & 1) + result * 2;
                temp >>= 1;
                temp2 -= 1;
                n -= 1;

                if n == 0 {
                    break;
                }
            }
            *z = temp;
            *y = temp2;
        }
        Ok(result)
    }
    fn decrypt_buf(&self, buf: &mut [u8]) {
        buf.iter_mut()
            .enumerate()
            .for_each(|(i, b)| *b ^= self.data[(i + 3) % 0x3FF])
    }
}

fn decrypt_with_password(
    buf: &[u8],
    size: usize,
    password: &[u8],
    key: u32,
) -> anyhow::Result<BytesMut> {
    let mut result = BytesMut::with_capacity(buf.len());
    let mut xor_buf = BytesMut::with_capacity(password.len());
    for chunk in password.chunks(4) {
        xor_buf.put_u32_le(chunk.pread::<u32>(0)?.wrapping_sub(key));
    }
    let mut k = key;
    k >>= 8;
    k ^= key;
    k >>= 8;
    k ^= key;
    k >>= 8;
    k ^= key;
    k ^= 0xFFFFFFFB;
    k &= 0x0F;
    k += 7;
    let xor_off = &mut 20;
    let data_off = &mut 0;
    for _ in 0..(size >> 2) {
        let mut v = xor_buf.gread_with::<u32>(xor_off, LE)?;
        v ^= buf.gread_with::<u32>(data_off, LE)?;
        v = v.wrapping_add(0x784C5062);
        v = v.rotate_right(k);
        v = v.wrapping_add(0x01010101);
        result.put_u32_le(v);
        *xor_off %= xor_buf.len();
    }

    for i in (size & 3)..0 {
        let mut v = xor_buf.gread_with::<u32>(xor_off, LE)?;
        v >>= i * 4;
        v ^= buf.gread_with::<u8>(data_off, LE)? as u32;
        v -= v.wrapping_sub(0x7D);
        result.put_u8(v as u8);
        *xor_off %= xor_buf.len();
    }
    Ok(result)
}

fn init_decrypt_table(key1: u32, key2: u32) -> Bytes {
    let mut table = BytesMut::with_capacity(0x100);
    for i in 0..=255 {
        table.put_u8(i);
    }
    let mut val = key1;
    for _ in 0..=255 {
        let mut x = val;
        x >>= 0x10;
        x &= 0xFF;
        let mut y = table[x as usize];
        let mut z = table[(val & 0xFF) as usize] as u32;
        table[(val & 0xFF) as usize] = y;
        table[x as usize] = z as u8;
        z = val;
        z >>= 8;
        z &= 0xFF;
        x = val;
        x >>= 0x18;
        y = table[x as usize];
        val = val.rotate_right(2);
        val = val.wrapping_mul(0x1A74F195);
        val = val.wrapping_add(key2);
        let a = table[z as usize];
        table[z as usize] = y;
        table[x as usize] = a;
    }
    table.freeze()
}

fn decrypt_with_decrypt_table(
    table: &[u8],
    data: &mut [u8],
    size: usize,
    xor_key: u8,
) {
    data.iter_mut()
        .take(size)
        .for_each(|b| *b = table[(*b ^ xor_key) as usize])
}

fn get_decrypt_buf(md5_cpz7: &[u8], key: u32) -> Bytes {
    let mut src = Bytes::copy_from_slice(&md5_cpz7);
    let mut dest = BytesMut::with_capacity(16);
    dest.put_u32_le(key.wrapping_add(0x76A3BF29) ^ src.get_u32_le());
    dest.put_u32_le(key ^ src.get_u32_le());
    dest.put_u32_le(key.wrapping_add(0x10000000) ^ src.get_u32_le());
    dest.put_u32_le(key ^ src.get_u32_le());
    dest.freeze()
}

fn decrypt_archive_data(
    decrypt_buf: &[u8],
    data: &[u8],
    key1: u32,
) -> anyhow::Result<Bytes> {
    let mut result = BytesMut::with_capacity(data.len());
    let mut e = 0x76548AEF;
    let decrypt_off = &mut 0;
    for chunk in data.chunks(4) {
        if chunk.len() == 4 {
            let mut b = decrypt_buf.gread_with::<u32>(decrypt_off, LE)?;
            b ^= chunk.pread_with::<u32>(0, LE)?;
            b = b.wrapping_sub(0x4A91C262);
            b = b.rotate_left(3);
            b = b.wrapping_sub(e);
            result.put_u32_le(b);

            *decrypt_off %= decrypt_buf.len();
            e = e.wrapping_add(key1 ^ 0x10FB562A);
        } else {
            for byte in chunk.iter() {
                let mut x = decrypt_buf.gread_with::<u32>(decrypt_off, LE)?;
                x >>= 6;
                x = (x as u8 ^ byte) as u32;
                x = x.wrapping_add(0x37);
                result.put_u8(x as u8);

                *decrypt_off %= decrypt_buf.len();
            }
        }
    }
    Ok(result.freeze())
}

fn decrypt_file_data(
    archive_data: &[ArchiveDataEntry],
    raw_file_data: &mut [u8],
    table: &[u8],
    md5_cpz7: &[u8],
    key2: u32,
) -> anyhow::Result<Bytes> {
    let mut result = BytesMut::with_capacity(raw_file_data.len());
    for (i, archive) in archive_data.iter().enumerate() {
        let offset = archive.offset;
        let mut size = raw_file_data.len() as u32;
        if i < archive_data.len() - 1 {
            size = archive_data[i + 1].offset;
        }
        size -= offset;
        decrypt_with_decrypt_table(
            &table,
            &mut raw_file_data[offset as usize..],
            size as usize,
            0x7E,
        );
        let decrypt_buf = get_decrypt_buf2(&md5_cpz7, archive.file_decrypt_key);
        let internal_data = internal_decrypt_file_data(
            &decrypt_buf,
            &raw_file_data[offset as usize..offset as usize + size as usize],
            key2,
        )?;
        result.extend(internal_data);
    }
    Ok(result.freeze())
}

fn get_decrypt_buf2(md5_cpz7: &[u8], key: u32) -> Bytes {
    let mut src = Bytes::copy_from_slice(&md5_cpz7);
    let mut dest = BytesMut::with_capacity(16);
    dest.put_u32_le(key ^ src.get_u32_le());
    dest.put_u32_le(key.wrapping_add(0x11003322) ^ src.get_u32_le());
    dest.put_u32_le(key ^ src.get_u32_le());
    dest.put_u32_le(key.wrapping_add(0x34216785) ^ src.get_u32_le());
    dest.freeze()
}

fn internal_decrypt_file_data(
    decrypt_buf: &[u8],
    data: &[u8],
    key2: u32,
) -> anyhow::Result<Bytes> {
    let mut result = BytesMut::with_capacity(data.len());
    let mut e = 0x2A65CB4F;
    let decrypt_off = &mut 0;
    for chunk in data.chunks(4) {
        if chunk.len() == 4 {
            let mut b = decrypt_buf.gread_with::<u32>(decrypt_off, LE)?;
            b ^= chunk.pread_with::<u32>(0, LE)?;
            b = b.wrapping_sub(e);
            b = b.rotate_left(2);
            b = b.wrapping_add(0x37A19E8B);
            result.put_u32_le(b);

            *decrypt_off %= decrypt_buf.len();
            e = e.wrapping_sub(key2 ^ 0x139FA9B);
        } else {
            for byte in chunk {
                let mut x = decrypt_buf.gread_with::<u32>(decrypt_off, LE)?;
                x >>= 4;
                x = (x as u8 ^ byte) as u32;
                x = x.wrapping_add(0x3);
                result.put_u8(x as u8);

                *decrypt_off %= decrypt_buf.len();
            }
        }
    }
    Ok(result.freeze())
}

fn get_file_key(
    file: &FileEntry,
    archive_file_decrypt_key: u32,
    header: &Cpz7Header,
    key3: u32,
    key4: u32,
) -> u32 {
    let mut file_key = file.file_decrypt_key;
    file_key = file_key.wrapping_add(archive_file_decrypt_key);
    file_key ^= header.archive_data_key;
    file_key = file_key.wrapping_add(header.archive_data_entry_count);
    file_key ^= key4;
    file_key = file_key.wrapping_sub(0x5C39E87B);
    file_key ^= header
        .file_decrypt_key
        .rotate_right(5)
        .wrapping_mul(0x7DA8F173)
        .wrapping_add(0x13712765)
        .wrapping_add(key3);
    file_key
}

fn decrypt_file(
    file_contents: &[u8],
    file_size: usize,
    md5_cpz7: &[u8],
    file_key: u32,
    table: &[u8],
    password: &[u8],
) -> anyhow::Result<Bytes> {
    let mut result = BytesMut::with_capacity(file_size);
    let v = md5_cpz7.pread_with::<u32>(4, LE)? >> 2;
    let mut decrypt_buf = BytesMut::with_capacity(password.len());
    for b in password {
        decrypt_buf.put_u8(table[*b as usize] ^ v as u8);
    }
    decrypt_buf.chunks_mut(4).for_each(|c| {
        c[0] ^= file_key as u8;
        c[1] ^= (file_key >> 8) as u8;
        c[2] ^= (file_key >> 16) as u8;
        c[3] ^= (file_key >> 24) as u8;
    });
    let mut c = 0x2748C39E;
    let decrypt_off = &mut 40;
    let mut dx = file_key;

    for chunk in file_contents.chunks(4) {
        if chunk.len() == 4 {
            let mut b = decrypt_buf.gread_with::<u32>(decrypt_off, LE)? >> 1;
            b ^= decrypt_buf.pread_with::<u32>(((c >> 6) & 0xF) * 4, LE)?;
            b ^= chunk.pread_with::<u32>(0, LE)?;
            b = b.wrapping_sub(dx);
            dx = c as u32 & 3;
            b ^= md5_cpz7.pread_with::<u32>(dx as usize * 4, LE)?;
            dx = file_key;
            result.put_u32_le(b);
            c = c.wrapping_add(file_key.wrapping_add(b) as usize);
            *decrypt_off &= 60;
        } else {
            for b in chunk {
                result.put_u8(table[(b ^ 0xAE) as usize]);
            }
        }
    }
    Ok(result.freeze())
}

fn md5_cpz7(buf: &[u8]) -> anyhow::Result<[u8; 16]> {
    let mut result = Bytes::copy_from_slice(&md5::compute(
        &buf,
        [0xC74A2B02, 0xE7C8AB8F, 0x38BEBC4E, 0x7531A4C3],
    ));
    let mut digest = BytesMut::with_capacity(16);
    let a = result.get_u32_le();
    let b = result.get_u32_le();
    let c = result.get_u32_le();
    let d = result.get_u32_le();
    digest.put_u32_le(c ^ 0x53A76D2E);
    digest.put_u32_le(b.wrapping_add(0x5BB17FDA));
    digest.put_u32_le(a.wrapping_add(0x6853E14D));
    digest.put_u32_le(d ^ 0xF5C6A9A3);
    Ok(digest.bytes().try_into()?)
}
