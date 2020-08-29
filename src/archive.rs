use bytes::Bytes;

// Workaround until it is possible to return impl Trait in traits
pub trait Archive {
    fn get_files(&self) -> Vec<FileEntry>;
    fn extract(&self, entry: &FileEntry) -> anyhow::Result<Bytes>;
    // fn extract_all(&self) -> anyhow::Result<()>;
}

// pub trait FileEntry: Debug {
//     fn file_name(&self) -> &str;
//     fn file_offset(&self) -> usize;
//     fn file_size(&self) -> usize;
// }

#[derive(Debug)]
pub struct FileEntry {
    pub file_name: String,
    pub file_offset: usize,
    pub file_size: usize,
}

#[derive(Debug)]
pub enum Entry {
    File(FileEntry),
    Directory(Vec<Entry>),
}
