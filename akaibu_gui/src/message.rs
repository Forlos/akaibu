use akaibu::{archive::FileEntry, scheme::Scheme};

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
    ClosePreview,
    PatternChanged(String),
    Error(String),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Scene {
    ArchiveView(Box<dyn Scheme>),
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Status {
    Normal(String),
    Success(String),
    Error(String),
    Empty,
}
