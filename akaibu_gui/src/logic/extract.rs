use super::convert;
use akaibu::archive::{Archive, FileEntry};
use anyhow::Context;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{path::PathBuf, sync::Arc};

pub async fn extract_single_file(
    archive: Arc<Box<dyn Archive>>,
    entry: FileEntry,
    file_path: PathBuf,
) -> anyhow::Result<PathBuf> {
    let file_contents = archive.extract(&entry)?;
    let mut output_file_name = PathBuf::from(
        file_path
            .parent()
            .context("Could not get parent directory")?,
    );
    output_file_name.push(&entry.file_name);
    log::info!("Extracting resource: {:?} {:X?}", output_file_name, entry);
    file_contents.write_contents(&output_file_name, Some(&archive))?;
    Ok(output_file_name)
}

pub async fn extract_all(
    archive: Arc<Box<dyn Archive>>,
    files: Vec<FileEntry>,
    file_path: PathBuf,
) -> anyhow::Result<PathBuf> {
    let mut extract_path = file_path
        .file_name()
        .context("Could not get file name")?
        .to_os_string();
    extract_path.push("_ext");
    let mut output_path = PathBuf::from(
        file_path
            .parent()
            .context("Could not get parent directory")?,
    );
    output_path.push(extract_path);
    files
        .par_iter()
        .try_for_each::<_, anyhow::Result<()>>(|entry| {
            let file_contents = archive.extract(entry)?;
            let mut output_file_path = output_path.clone();
            output_file_path.push(&entry.full_path);
            std::fs::create_dir_all(
                &output_file_path
                    .parent()
                    .context("Could not get parent directory")?,
            )?;
            log::info!(
                "Extracting resource: {:?} {:X?}",
                output_file_path,
                entry
            );
            file_contents.write_contents(&output_file_path, Some(&archive))?;
            Ok(())
        })?;
    Ok(output_path)
}

pub async fn extract_all_with_convert(
    archive: Arc<Box<dyn Archive>>,
    files: Vec<FileEntry>,
    file_path: PathBuf,
) -> anyhow::Result<PathBuf> {
    let mut extract_path = file_path
        .file_name()
        .context("Could not get file name")?
        .to_os_string();
    extract_path.push("_ext");
    let mut output_path = PathBuf::from(
        file_path
            .parent()
            .context("Could not get parent directory")?,
    );
    output_path.push(extract_path);
    files
        .par_iter()
        .try_for_each::<_, anyhow::Result<()>>(|entry| {
            match convert::convert_resource_blocking(
                &archive,
                &entry,
                &output_path,
            ) {
                Ok(_) => Ok(()),
                Err(_) => {
                    let file_contents = archive.extract(entry)?;
                    let mut output_file_path = output_path.clone();
                    output_file_path.push(&entry.full_path);
                    std::fs::create_dir_all(
                        &output_file_path
                            .parent()
                            .context("Could not get parent directory")?,
                    )?;
                    log::info!(
                        "Extracting resource: {:?} {:X?}",
                        output_file_path,
                        entry
                    );
                    file_contents
                        .write_contents(&output_file_path, Some(&archive))?;
                    Ok(())
                }
            }
        })?;
    Ok(output_path)
}
