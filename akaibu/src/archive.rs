use bytes::Bytes;
use std::{collections::HashMap, ffi::OsStr, fmt::Debug, path::PathBuf};

// Workaround until it is possible to return impl Trait in traits
pub trait Archive {
    fn get_files(&self) -> Vec<FileEntry>;
    fn extract(&self, entry: &FileEntry) -> anyhow::Result<Bytes>;
    fn extract_all(&self, output_path: &PathBuf) -> anyhow::Result<()>;
    fn get_root_dir(&self) -> &Directory;
}

// pub trait FileEntry: Debug {
//     fn file_name(&self) -> &str;
//     fn file_offset(&self) -> usize;
//     fn file_size(&self) -> usize;
// }

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub file_name: String,
    pub full_path: PathBuf,
    pub file_offset: u64,
    pub file_size: u64,
}

#[derive(Debug)]
pub struct Directory {
    pub files: Vec<FileEntry>,
    pub directories: HashMap<String, Directory>,
}

impl Directory {
    pub fn new(files: Vec<FileEntry>) -> Self {
        let mut root_dir = Directory {
            files: Vec::new(),
            directories: HashMap::new(),
        };
        for entry in files {
            let dirs = entry.full_path.iter().collect::<Vec<&OsStr>>();
            let mut current = &mut root_dir;
            if dirs.len() == 1 {
                current.files.push(entry);
            } else {
                for dir in &dirs[..dirs.len() - 1] {
                    current = current
                        .directories
                        .entry(String::from(
                            dir.to_str().expect("Not valid UTF-8"),
                        ))
                        .or_insert(Directory {
                            files: Vec::new(),
                            directories: HashMap::new(),
                        });
                }
                current.files.push(entry);
            }
        }
        root_dir
    }
    pub fn get_all_files<'a>(
        &'a self,
    ) -> Box<dyn Iterator<Item = &FileEntry> + 'a> {
        Box::new(
            self.files.iter().chain(
                self.directories
                    .values()
                    .map(|directory| directory.get_all_files())
                    .flatten(),
            ),
        )
    }
}