use akaibu::archive::FileEntry;

#[derive(Debug, Clone)]
pub(crate) enum Message {
    ExtractAll,
    UpdateScrollbar(f32),
    ExtractFile(FileEntry),
    PreviewFile(FileEntry),
    Error(String),
    Empty,
}
