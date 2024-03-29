use crate::archive;
use archive::NavigableDirectory;
use dyn_clone::DynClone;
use std::{fmt::Debug, path::Path};

pub mod acv1;
pub mod amusepac;
pub mod buriko;
pub mod cpz7;
pub mod esc_arc2;
pub mod gxp;
pub mod iar;
pub mod link6;
pub mod malie;
pub mod nekopack;
pub mod pf8;
pub mod qliepack;
pub mod silky;
pub mod tactics_arc;
pub mod willplus_arc;
pub mod ypf;

pub trait Scheme: Debug + Send + DynClone {
    fn extract(
        &self,
        file_path: &Path,
    ) -> anyhow::Result<(Box<dyn archive::Archive>, NavigableDirectory)>;
    fn get_name(&self) -> String;
    fn get_schemes() -> Vec<Box<dyn Scheme>>
    where
        Self: Sized;
}

dyn_clone::clone_trait_object!(Scheme);
