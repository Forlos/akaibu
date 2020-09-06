use akaibu::archive::FileEntry;

#[derive(Debug, Clone)]
pub enum Message {
    MoveScene(Scene),
    ExtractAll,
    UpdateScrollbar(f32),
    OpenDirectory(String),
    BackDirectory,
    ExtractFile(FileEntry),
    PreviewFile(FileEntry),
    Error(String),
    Empty,
}

#[derive(Debug, Clone)]
pub enum Scene {
    ArchiveView,
}
