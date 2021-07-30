use crate::archive;

use super::{ResourceScheme, ResourceType};
use libwebp_image::webp_load_from_memory;
use scroll::{Pread, LE};
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, Clone)]
pub(crate) enum PnaScheme {
    Universal,
}

#[derive(Debug, Pread)]
struct PnaHeader {
    magic: [u8; 4],
    unk0: u32,
    unk1: u32,
    unk2: u32,
    entry_count: u32,
}

#[derive(Debug, Pread)]
struct PnaEntry {
    typ: u32,
    id: u32,
    left_offset: u32,
    top_offset: u32,
    width: u32,
    height: u32,
    unk0: u32,
    unk1: u32,
    unk2: u32,
    size: u32,
}

impl ResourceScheme for PnaScheme {
    fn convert(
        &self,
        file_path: &std::path::Path,
    ) -> anyhow::Result<super::ResourceType> {
        let mut buf = Vec::with_capacity(1 << 20);
        let mut file = File::open(file_path)?;
        file.read_to_end(&mut buf)?;
        self.from_bytes(buf, file_path)
    }

    fn convert_from_bytes(
        &self,
        file_path: &std::path::Path,
        buf: Vec<u8>,
        _archive: Option<&Box<dyn archive::Archive>>,
    ) -> anyhow::Result<super::ResourceType> {
        self.from_bytes(buf, file_path)
    }

    fn get_name(&self) -> String {
        format!(
            "[PNA] {}",
            match self {
                Self::Universal => "Universal",
            }
        )
    }

    fn get_schemes() -> Vec<Box<dyn ResourceScheme>>
    where
        Self: Sized,
    {
        vec![Box::new(Self::Universal)]
    }
}

impl PnaScheme {
    fn from_bytes(
        &self,
        buf: Vec<u8>,
        _file_path: &Path,
    ) -> anyhow::Result<ResourceType> {
        let off = &mut 0;
        let header = buf.gread_with::<PnaHeader>(off, LE)?;
        let mut entries = Vec::with_capacity(header.entry_count as usize);
        for _ in 0..header.entry_count {
            let entry = buf.gread_with::<PnaEntry>(off, LE)?;
            if entry.size > 0 {
                entries.push(entry);
            }
        }
        let mut images = Vec::with_capacity(header.entry_count as usize);
        for entry in entries.iter() {
            let size = entry.size as usize;
            let image = match &header.magic {
                b"PNAP" => image::load_from_memory_with_format(
                    &buf[*off..*off + size],
                    image::ImageFormat::Png,
                )?,
                b"WPAP" => webp_load_from_memory(&buf[*off..*off + size])?,
                _ => {
                    return Err(crate::error::AkaibuError::Custom(format!(
                        "Unsupported format {} {:X?}",
                        String::from_utf8_lossy(&header.magic),
                        header.magic
                    ))
                    .into());
                }
            };
            *off += size;
            images.push(image.to_rgba8());
        }
        Ok(ResourceType::SpriteSheet { sprites: images })
    }
}
