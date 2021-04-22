use std::sync::Arc;

use akaibu::{
    archive::Archive, archive::FileEntry, resource::ResourceMagic,
    resource::ResourceType,
};
use anyhow::Context;

pub async fn get_resource_type(
    archive: Arc<Box<dyn Archive>>,
    entry: FileEntry,
) -> anyhow::Result<ResourceType> {
    let contents = archive.extract(&entry)?;
    let resource_magic = ResourceMagic::parse_magic(&contents);
    resource_magic
        .get_schemes()
        .get(0)
        .context("Unknown resource format")?
        .convert_from_bytes(&entry.full_path, contents.to_vec())
}
