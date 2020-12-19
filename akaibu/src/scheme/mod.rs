use crate::archive;
use archive::NavigableDirectory;
use dyn_clone::DynClone;
use std::{fmt::Debug, path::PathBuf};

pub mod acv1;
pub mod buriko;
pub mod cpz7;
pub mod esc_arc2;
pub mod gxp;
pub mod pf8;
pub mod ypf;

pub trait Scheme: Debug + Send + DynClone {
    fn extract(
        &self,
        file_path: &PathBuf,
    ) -> anyhow::Result<(Box<dyn archive::Archive + Sync>, NavigableDirectory)>;
    fn get_name(&self) -> &str;
    fn get_schemes() -> Vec<Box<dyn Scheme>>
    where
        Self: Sized;
}

dyn_clone::clone_trait_object!(Scheme);
