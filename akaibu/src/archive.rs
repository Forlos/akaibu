use bytes::Bytes;
use itertools::Itertools;
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fmt::Debug,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

use crate::resource::ResourceMagic;

// Workaround until it is possible to return impl Trait in traits
pub trait Archive: Sync + Send + Debug {
    fn extract(&self, entry: &FileEntry) -> anyhow::Result<FileContents>;
    fn extract_all(&self, output_path: &Path) -> anyhow::Result<()>;
}

// pub trait FileEntry: Debug {
//     fn file_name(&self) -> &str;
//     fn file_offset(&self) -> usize;
//     fn file_size(&self) -> usize;
// }
//

#[derive(Debug)]
pub struct FileContents {
    pub contents: Bytes,
    pub type_hint: Option<ResourceMagic>,
}

impl FileContents {
    pub fn get_resource_type(&self) -> ResourceMagic {
        if let Some(resource_type) = &self.type_hint {
            resource_type.clone()
        } else {
            ResourceMagic::parse_magic(&self.contents)
        }
    }
    pub fn write_contents(
        &self,
        output_file_name: &Path,
        archive: Option<&Box<dyn Archive>>,
    ) -> anyhow::Result<()> {
        if let Some(resource_type) = &self.type_hint {
            let resource = resource_type
                .get_schemes()
                .get(0)
                .expect("Expected universal scheme")
                .convert_from_bytes(
                    &PathBuf::new(),
                    self.contents.to_vec(),
                    archive,
                )?;
            resource.write_resource(&output_file_name)?;
        } else {
            File::create(output_file_name)?.write_all(&self.contents)?;
        };
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub file_name: String,
    pub full_path: PathBuf,
    pub file_offset: u64,
    pub file_size: u64,
}

#[derive(Debug, Clone)]
pub struct Directory {
    pub files: Vec<FileEntry>,
    pub directories: BTreeMap<String, Directory>,
}

impl Directory {
    pub fn new(files: Vec<FileEntry>) -> Self {
        let mut root_dir = Directory {
            files: Vec::new(),
            directories: BTreeMap::new(),
        };
        for entry in files
            .into_iter()
            .sorted_by(|a, b| a.full_path.cmp(&b.full_path))
        {
            let dirs = entry.full_path.iter().collect::<Vec<&OsStr>>();
            let mut current = &mut root_dir;
            if dirs.len() <= 1 {
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
                            directories: BTreeMap::new(),
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
    pub fn find_dir(&self, dir_names: &[String]) -> Option<&Directory> {
        if dir_names.is_empty() {
            Some(&self)
        } else {
            self.directories
                .get(&dir_names[0])?
                .find_dir(&dir_names[1..])
        }
    }
}

#[derive(Debug)]
pub struct NavigableDirectory {
    root_dir: Directory,
    current: Vec<String>,
}
impl NavigableDirectory {
    pub fn new(root_dir: Directory) -> Self {
        Self {
            root_dir,
            current: Vec::new(),
        }
    }
    pub fn get_root_dir(&self) -> &Directory {
        &self.root_dir
    }
    pub fn get_current(&self) -> &Directory {
        self.root_dir
            .find_dir(&self.current)
            .expect("Could not get current dir")
    }
    pub fn move_dir(&mut self, dir: &str) -> Option<&Directory> {
        self.current.push(dir.to_string());
        self.root_dir.find_dir(&self.current.as_slice())
    }
    pub fn back_dir(&mut self) -> Option<&Directory> {
        self.current.pop()?;
        self.root_dir.find_dir(&self.current)
    }
    pub fn get_current_full_path(&self) -> String {
        self.current
            .iter()
            .fold(String::from("/"), |mut path, dir| {
                path.push_str(&format!("{}/", dir));
                path
            })
    }
    pub fn has_parent(&self) -> bool {
        !self.current.is_empty()
    }
}
