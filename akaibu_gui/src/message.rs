use akaibu::{archive::FileEntry, scheme::Scheme};

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
    Error(String),
    Empty,
}

#[derive(Debug, Clone)]
pub enum Scene {
    ArchiveView(Box<dyn Scheme>),
}
