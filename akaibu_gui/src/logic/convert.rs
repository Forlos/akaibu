use crate::ui::resource::ConvertFormat;
use akaibu::{
    archive::Archive, archive::FileEntry, resource::ResourceMagic,
    resource::ResourceType,
};
use anyhow::Context;
use image::ImageFormat;
use std::{fs::File, io::Write, path::PathBuf, sync::Arc};

pub async fn convert_resource(
    archive: Arc<Box<dyn Archive>>,
    entry: FileEntry,
    file_path: PathBuf,
) -> anyhow::Result<PathBuf> {
    let contents = archive.extract(&entry)?;
    let resource_magic = ResourceMagic::parse_magic(&contents);
    log::info!("Converting resource {:?}", resource_magic);
    let mut converted_path = file_path;
    converted_path.set_file_name(&entry.file_name);
    write_resource(
        resource_magic
            .get_schemes()
            .get(0)
            .context("Expected universal scheme")?
            .convert_from_bytes(&converted_path, contents.to_vec())?,
        &entry,
        &converted_path,
    )?;
    Ok(converted_path)
}

#[allow(clippy::borrowed_box)]
pub fn convert_resource_blocking(
    archive: &Box<dyn Archive>,
    entry: &FileEntry,
    file_path: &PathBuf,
) -> anyhow::Result<PathBuf> {
    let contents = archive.extract(&entry)?;
    let resource_magic = ResourceMagic::parse_magic(&contents);
    log::info!("Converting resource {:?}", resource_magic);
    let mut converted_path = file_path.clone();
    converted_path.set_file_name(&entry.file_name);
    write_resource_entry(
        resource_magic
            .get_schemes()
            .get(0)
            .context("Expected universal scheme")?
            .convert_from_bytes(&converted_path, contents.to_vec())?,
        &entry,
        file_path,
    )?;
    Ok(converted_path)
}

fn write_resource(
    resource: ResourceType,
    entry: &FileEntry,
    file_name: &PathBuf,
) -> anyhow::Result<()> {
    match resource {
        ResourceType::SpriteSheet { mut sprites } => {
            if sprites.len() == 1 {
                let image = sprites.remove(0);
                let mut new_file_name = file_name.clone();
                new_file_name.set_extension("png");
                image.save(new_file_name)?;
            } else {
                for (i, sprite) in sprites.iter().enumerate() {
                    let mut new_file_name = file_name.clone();
                    new_file_name.set_file_name(format!(
                        "{}_{}",
                        new_file_name
                            .file_stem()
                            .context("Could not get file name")?
                            .to_str()
                            .context("Not valid UTF-8")?,
                        i
                    ));
                    new_file_name.set_extension("png");
                    sprite.save(&new_file_name)?;
                }
            }
            Ok(())
        }
        ResourceType::RgbaImage { image } => {
            let mut new_file_name = file_name.clone();
            new_file_name.set_extension("png");
            image.save(new_file_name)?;
            Ok(())
        }
        ResourceType::Text(s) => {
            let mut new_file_name = file_name.clone();
            new_file_name.set_extension("txt");
            File::create(new_file_name)?.write_all(s.as_bytes())?;
            Ok(())
        }
        ResourceType::Other => Err(akaibu::error::AkaibuError::Custom(
            format!("Convert not available for: {}", entry.file_name),
        )
        .into()),
    }
}

pub fn write_resource_with_format(
    resource: ResourceType,
    mut file_name: PathBuf,
    format: ConvertFormat,
) -> anyhow::Result<PathBuf> {
    match resource {
        ResourceType::RgbaImage { image } => {
            file_name.set_extension(format!("{}", format));
            image.save_with_format(
                &file_name,
                match format {
                    ConvertFormat::PNG => ImageFormat::Png,
                    ConvertFormat::JPEG => ImageFormat::Jpeg,
                    ConvertFormat::BMP => ImageFormat::Bmp,
                    ConvertFormat::TIFF => ImageFormat::Tiff,
                    ConvertFormat::ICO => ImageFormat::Ico,
                },
            )?;
            Ok(file_name)
        }
        _ => Err(akaibu::error::AkaibuError::Custom(format!(
            "Convert not available for: {:?}",
            file_name
        ))
        .into()),
    }
}

fn write_resource_entry(
    resource: ResourceType,
    entry: &FileEntry,
    file_path: &PathBuf,
) -> anyhow::Result<()> {
    match resource {
        ResourceType::SpriteSheet { mut sprites } => {
            if sprites.len() == 1 {
                let image = sprites.remove(0);
                let mut new_file_name = file_path.clone();
                new_file_name.set_extension("png");
                image.save(new_file_name)?;
            } else {
                for (i, sprite) in sprites.iter().enumerate() {
                    let mut new_file_name = file_path.clone();
                    new_file_name.set_file_name(format!(
                        "{}_{}",
                        new_file_name
                            .file_stem()
                            .context("Could not get file name")?
                            .to_str()
                            .context("Not valid UTF-8")?,
                        i
                    ));
                    new_file_name.set_extension("png");
                    sprite.save(&new_file_name)?;
                }
            }
            Ok(())
        }
        ResourceType::RgbaImage { image } => {
            let mut new_file_name = file_path.clone();
            new_file_name.push(entry.full_path.clone());
            new_file_name.set_extension("png");
            image.save(new_file_name)?;
            Ok(())
        }
        ResourceType::Text(s) => {
            let mut new_file_name = file_path.clone();
            new_file_name.push(entry.full_path.clone());
            new_file_name.set_extension("txt");
            File::create(new_file_name)?.write_all(s.as_bytes())?;
            Ok(())
        }
        ResourceType::Other => Err(akaibu::error::AkaibuError::Unimplemented(
            format!("Convert not available for: {}", entry.file_name),
        )
        .into()),
    }
}
