use std::{fs::File, io::Write, path::PathBuf, sync::Arc};

use akaibu::archive::{Archive, FileEntry};
use anyhow::Context;
use iced::{futures, Command};
use rayon::iter::{
    IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator,
};

use crate::message::Message;

pub async fn extract_single_file(
    archive: Arc<Box<dyn Archive>>,
    entry: FileEntry,
    file_path: PathBuf,
) -> anyhow::Result<PathBuf> {
    let buf = archive.extract(&entry)?;
    let mut output_file_name = PathBuf::from(
        file_path
            .parent()
            .context("Could not get parent directory")?,
    );
    output_file_name.push(&entry.file_name);
    log::info!("Extracting resource: {:?} {:X?}", output_file_name, entry);
    File::create(&output_file_name)?.write_all(&buf)?;
    Ok(output_file_name)
}

pub async fn extract_all(
    archive: Arc<Box<dyn Archive>>,
    files: Vec<FileEntry>,
    file_path: PathBuf,
) -> anyhow::Result<()> {
    let files_count = files.len();
    files.par_iter().enumerate().try_for_each(|(i, entry)| {
        let buf = archive.extract(entry)?;
        let mut output_file_name = PathBuf::from(
            file_path
                .parent()
                .context("Could not get parent directory")?,
        );
        let mut extract_path = file_path
            .file_name()
            .context("Could not get file name")?
            .to_os_string();
        extract_path.push("_ext");
        output_file_name.push(extract_path);
        output_file_name.push(&entry.full_path);
        std::fs::create_dir_all(
            &output_file_name
                .parent()
                .context("Could not get parent directory")?,
        )?;
        // log::info!("Extracting resource: {:?} {:X?}", output_file_name, entry);
        File::create(output_file_name)?.write_all(&buf)?;
        Command::perform(
            futures::future::ready((i / files_count) as f32 * 100.0),
            Message::UpdateScrollbar,
        );
        Ok(())
    })
}
