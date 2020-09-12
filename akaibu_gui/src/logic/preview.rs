use akaibu::{
    archive::Archive, archive::FileEntry, resource::ResourceMagic,
    resource::ResourceType,
};

pub fn get_resource_type(
    archive: &Box<dyn Archive>,
    entry: &FileEntry,
) -> anyhow::Result<ResourceType> {
    let contents = archive.extract(&entry)?;
    let resource_magic = ResourceMagic::parse_magic(&contents);
    log::info!("Converting resource {:?}", resource_magic);
    resource_magic.parse(contents.to_vec())
}
