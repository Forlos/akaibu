use super::{ResourceScheme, ResourceType};
use crate::error::AkaibuError;
use scroll::Pread;
use std::{fs::File, io::Read, path::Path};
use tlg_rs::formats::{tlg0::Tlg0, tlg6::Tlg6};

#[derive(Debug, Clone)]
pub(crate) enum TlgScheme {
    Universal,
}

impl ResourceScheme for TlgScheme {
    fn convert(&self, file_path: &Path) -> anyhow::Result<super::ResourceType> {
        let mut buf = Vec::with_capacity(1 << 20);
        let mut file = File::open(file_path)?;
        file.read_to_end(&mut buf)?;
        parse_tlg(buf)
    }

    fn convert_from_bytes(
        &self,
        _file_path: &Path,
        buf: Vec<u8>,
    ) -> anyhow::Result<ResourceType> {
        parse_tlg(buf)
    }

    fn get_name(&self) -> String {
        format!(
            "[TLG] {}",
            match self {
                Self::Universal => "Universal",
            }
        )
    }

    fn get_schemes() -> Vec<Box<dyn ResourceScheme>>
    where
        Self: Sized,
    {
        vec![Box::new(Self::Universal)]
    }
}

fn parse_tlg(buf: Vec<u8>) -> anyhow::Result<ResourceType> {
    let image = match buf.pread::<u8>(3)? - 0x30 {
        0 => Tlg0::from_bytes(&buf)?.to_rgba_image()?,
        6 => Tlg6::from_bytes(&buf)?.to_rgba_image()?,
        ver => {
            return Err(AkaibuError::Unimplemented(format!(
                "Version: {} is not supported",
                ver
            ))
            .into())
        }
    };
    Ok(ResourceType::RgbaImage { image })
}
