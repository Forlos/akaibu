use std::sync::Arc;

use akaibu::{archive::Archive, archive::FileEntry, resource::ResourceType};
use anyhow::Context;

pub async fn get_resource_type(
    archive: Arc<Box<dyn Archive>>,
    entry: FileEntry,
) -> anyhow::Result<ResourceType> {
    let file_contents = archive.extract(&entry)?;
    file_contents
        .get_resource_type()
        .get_schemes()
        .get(0)
        .context("Unknown resource format")?
        .convert_from_bytes(&entry.full_path, file_contents.contents.to_vec())
}
