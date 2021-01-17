use super::{ResourceScheme, ResourceType};
use crate::error::AkaibuError;
use std::{fs::File, io::Read};
use tlg_rs::formats::{tlg0::Tlg0, tlg6::Tlg6};

#[derive(Debug, Clone)]
pub(crate) enum Tlg0Scheme {
    Universal,
}

impl ResourceScheme for Tlg0Scheme {
    fn convert(
        &self,
        file_path: &std::path::PathBuf,
    ) -> anyhow::Result<super::ResourceType> {
        let mut buf = Vec::with_capacity(1 << 20);
        let mut file = File::open(file_path)?;
        file.read_to_end(&mut buf)?;
        let image = Tlg0::from_bytes(&buf)?.to_rgba_image()?;
        Ok(ResourceType::RgbaImage { image })
    }

    fn convert_from_bytes(
        &self,
        _file_path: &std::path::PathBuf,
        buf: Vec<u8>,
    ) -> anyhow::Result<ResourceType> {
        let image = Tlg0::from_bytes(&buf)?.to_rgba_image()?;
        Ok(ResourceType::RgbaImage { image })
    }

    fn get_name(&self) -> String {
        format!(
            "[TLG0] {}",
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

#[derive(Debug, Clone)]
pub(crate) enum Tlg5Scheme {
    Universal,
}

impl ResourceScheme for Tlg5Scheme {
    fn convert(
        &self,
        _file_path: &std::path::PathBuf,
    ) -> anyhow::Result<ResourceType> {
        Err(
            AkaibuError::Unimplemented(String::from("TLG5 is not supported"))
                .into(),
        )
    }
    fn convert_from_bytes(
        &self,
        _file_path: &std::path::PathBuf,
        _buf: Vec<u8>,
    ) -> anyhow::Result<ResourceType> {
        Err(
            AkaibuError::Unimplemented(String::from("TLG5 is not supported"))
                .into(),
        )
    }

    fn get_name(&self) -> String {
        format!(
            "[TLG5] {}",
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

#[derive(Debug, Clone)]
pub(crate) enum Tlg6Scheme {
    Universal,
}

impl ResourceScheme for Tlg6Scheme {
    fn convert(
        &self,
        file_path: &std::path::PathBuf,
    ) -> anyhow::Result<super::ResourceType> {
        let mut buf = Vec::with_capacity(1 << 20);
        let mut file = File::open(file_path)?;
        file.read_to_end(&mut buf)?;
        let image = Tlg6::from_bytes(&buf)?.to_rgba_image()?;
        Ok(ResourceType::RgbaImage { image })
    }
    fn convert_from_bytes(
        &self,
        _file_path: &std::path::PathBuf,
        buf: Vec<u8>,
    ) -> anyhow::Result<ResourceType> {
        let image = Tlg6::from_bytes(&buf)?.to_rgba_image()?;
        Ok(ResourceType::RgbaImage { image })
    }

    fn get_name(&self) -> String {
        format!(
            "[TLG6] {}",
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
