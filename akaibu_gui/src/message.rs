use crate::ui::resource::ConvertFormat;
use akaibu::{
    archive::FileEntry,
    resource::{ResourceScheme, ResourceType},
    scheme::Scheme,
};
use std::path::PathBuf;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Message {
    MoveScene(Scene),
    ExtractAll,
    UpdateScrollbar(f32),
    OpenDirectory(String),
    BackDirectory,
    ConvertFile(FileEntry),
    ExtractFile(FileEntry),
    PreviewFile(FileEntry),
    SetStatus(Status),
    OpenPreview(ResourceType, String),
    ClosePreview,
    ConvertAllToggle(bool),
    PatternChanged(String),
    FormatChanged(ConvertFormat),
    SaveResource,
    Error(String),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Scene {
    ArchiveView(Box<dyn Scheme>),
    ResourceView(Box<dyn ResourceScheme>, PathBuf),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Status {
    Normal(String),
    Success(String),
    Error(String),
    Empty,
}
