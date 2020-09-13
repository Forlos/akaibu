use crate::{
    message::Message,
    ui::{archive::ArchiveContent, scheme::SchemeContent},
};
use iced::Element;

pub enum Content {
    SchemeView(SchemeContent),
    ArchiveView(Box<ArchiveContent>),
}

impl Content {
    pub fn view(&mut self) -> Element<Message> {
        match self {
            Content::ArchiveView(content) => content.view(),
            Content::SchemeView(content) => content.view(),
        }
    }
}
