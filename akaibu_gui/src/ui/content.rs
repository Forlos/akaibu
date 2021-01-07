use crate::{
    message::Message,
    ui::{
        archive::ArchiveContent, resource::ResourceContent,
        scheme::SchemeContent,
    },
};
use iced::Element;

pub enum Content {
    SchemeView(SchemeContent),
    ArchiveView(Box<ArchiveContent>),
    ResourceView(ResourceContent),
}

impl Content {
    pub fn view(&mut self) -> Element<'_, Message> {
        match self {
            Content::ArchiveView(content) => content.view(),
            Content::SchemeView(content) => content.view(),
            Content::ResourceView(content) => content.view(),
        }
    }
}
