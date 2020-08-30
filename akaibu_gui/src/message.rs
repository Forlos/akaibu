#[derive(Debug, Clone)]
pub(crate) enum Message {
    ExtractAll,
    UpdateScrollbar(f32),
    ExtractFile(String),
    PreviewFile(String),
    Error(String),
    Empty,
}
