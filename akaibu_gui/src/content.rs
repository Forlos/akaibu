use crate::{archive::ArchiveContent, message::Message, scheme::SchemeContent};
use iced::Element;

pub enum Content {
    SchemeView(SchemeContent),
    ArchiveView(ArchiveContent),
}

impl Content {
    pub fn view(&mut self) -> Element<Message> {
        match self {
            Content::ArchiveView(content) => content.view(),
            Content::SchemeView(content) => content.view(),
        }
    }
}
