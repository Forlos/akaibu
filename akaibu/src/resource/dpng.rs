use crate::archive;

use super::{ResourceScheme, ResourceType};
use image::{ImageBuffer, RgbaImage};
use scroll::{Pread, LE};
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, Clone)]
pub(crate) enum DpngScheme {
    Universal,
}

#[derive(Debug, Pread)]
struct DpngHeader {
    magic: [u8; 4],
    unk0: u32,
    entry_count: u32,
    width: u32,
    height: u32,
}

#[derive(Debug, Pread)]
struct DpngEntry {
    left_offset: u32,
    top_offset: u32,
    width: u32,
    height: u32,
    data_size: u32,
    unk1: u32,
    unk2: u32,
}

impl ResourceScheme for DpngScheme {
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
            "[DPNG] {}",
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

impl DpngScheme {
    fn from_bytes(
        &self,
        buf: Vec<u8>,
        _file_path: &Path,
    ) -> anyhow::Result<ResourceType> {
        let off = &mut 0;
        let header = buf.gread_with::<DpngHeader>(off, LE)?;
        let mut entries = Vec::with_capacity(header.entry_count as usize);
        for _ in 0..header.entry_count {
            let entry = buf.gread_with::<DpngEntry>(off, LE)?;
            if entry.data_size > 0 {
                let image = image::load_from_memory_with_format(
                    &buf[*off..*off + entry.data_size as usize],
                    image::ImageFormat::Png,
                )?
                .to_rgba8();
                *off += entry.data_size as usize;
                entries.push((entry, image));
            }
        }
        let mut combined_image: RgbaImage =
            ImageBuffer::new(header.width, header.height);

        for (entry, image) in entries {
            for x in 0..entry.width {
                for y in 0..entry.height {
                    combined_image.put_pixel(
                        x + entry.left_offset,
                        y + entry.top_offset,
                        *image.get_pixel(x, y),
                    );
                }
            }
        }

        Ok(ResourceType::RgbaImage {
            image: combined_image,
        })
    }
}
