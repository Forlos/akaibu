use super::{ResourceScheme, ResourceType};
use crate::archive;
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, Clone)]
pub(crate) struct Common(pub(crate) String);

impl ResourceScheme for Common {
    fn convert(&self, file_path: &Path) -> anyhow::Result<ResourceType> {
        let mut buf = Vec::with_capacity(1 << 20);
        let mut file = File::open(file_path)?;
        file.read_to_end(&mut buf)?;
        self.convert_from_bytes(file_path, buf, None)
    }

    fn convert_from_bytes(
        &self,
        _file_path: &Path,
        buf: Vec<u8>,
        _archive: Option<&Box<dyn archive::Archive>>,
    ) -> anyhow::Result<ResourceType> {
        Ok(ResourceType::RgbaImage {
            image: image::load_from_memory(&buf)?.to_rgba8(),
        })
    }

    fn get_name(&self) -> String {
        format!("[Common File Formats] {}", self.0)
    }

    fn get_schemes() -> Vec<Box<dyn ResourceScheme>>
    where
        Self: Sized,
    {
        vec![Box::new(Self("".to_string()))]
    }
}
