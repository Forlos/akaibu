use anyhow::Context;
use image::ImageBuffer;
use std::{fs::File, io::Write, path::PathBuf};

use akaibu::{
    archive::Archive, archive::FileEntry, resource::ResourceMagic,
    resource::ResourceType,
};

pub fn convert_resource(
    archive: &Box<dyn Archive>,
    entry: &FileEntry,
    file_path: &PathBuf,
) -> anyhow::Result<PathBuf> {
    let contents = archive.extract(&entry)?;
    let resource_magic = ResourceMagic::parse_magic(&contents);
    log::info!("Converting resource {:?}", resource_magic);
    let mut converted_path = file_path.clone();
    converted_path.set_file_name(&entry.file_name);
    write_resource(resource_magic.parse(contents.to_vec())?, &converted_path)?;
    Ok(converted_path)
}

fn write_resource(
    resource: ResourceType,
    file_name: &PathBuf,
) -> anyhow::Result<()> {
    match resource {
        ResourceType::RgbaImage {
            pixels,
            width,
            height,
        } => {
            let mut new_file_name = file_name.clone();
            new_file_name.set_extension("png");
            let image: ImageBuffer<image::Rgba<u8>, Vec<u8>> =
                ImageBuffer::from_raw(width, height, pixels)
                    .context("Could not create image")?;
            image.save(new_file_name)?;
            Ok(())
        }
        ResourceType::Text(s) => {
            let mut new_file_name = file_name.clone();
            new_file_name.set_extension("txt");
            File::create(new_file_name)?.write_all(s.as_bytes())?;
            Ok(())
        }
        ResourceType::Other => Ok(()),
    }
}
