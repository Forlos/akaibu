use bytes::Bytes;

// Workaround until it is possible to return impl Trait in traits
pub trait Archive {
    fn get_files(&self) -> Vec<FileEntry<'_>>;
    fn extract(&self, file_name: &str) -> anyhow::Result<Bytes>;
}

// pub trait FileEntry: Debug {
//     fn file_name(&self) -> &str;
//     fn file_offset(&self) -> usize;
//     fn file_size(&self) -> usize;
// }

#[derive(Debug)]
pub struct FileEntry<'a> {
    pub file_name: &'a str,
    pub file_offset: usize,
    pub file_size: usize,
}
