use std::{fs::File, io::Read, path::Path};

use super::{ResourceScheme, ResourceType};

#[derive(Debug, Clone)]
pub(crate) struct Common;

impl ResourceScheme for Common {
    fn convert(&self, file_path: &Path) -> anyhow::Result<ResourceType> {
        let mut buf = Vec::with_capacity(1 << 20);
        let mut file = File::open(file_path)?;
        file.read_to_end(&mut buf)?;
        self.convert_from_bytes(file_path, buf)
    }

    fn convert_from_bytes(
        &self,
        _file_path: &Path,
        buf: Vec<u8>,
    ) -> anyhow::Result<ResourceType> {
        Ok(ResourceType::RgbaImage {
            image: image::load_from_memory(&buf)?.to_rgba8(),
        })
    }

    fn get_name(&self) -> String {
        String::from("Common")
    }

    fn get_schemes() -> Vec<Box<dyn ResourceScheme>>
    where
        Self: Sized,
    {
        vec![Box::new(Self)]
    }
}
