use crate::{
    archive::ArchiveContent,
    message::{Message, Scene},
    scheme::SchemeContent,
    style,
};
use iced::{button, Button, Container, Element, Length, Text};

pub enum Content {
    Empty(button::State),
    SchemeView(SchemeContent),
    ArchiveView(ArchiveContent),
}

impl Content {
    pub fn view(&mut self) -> Element<Message> {
        match self {
            Content::Empty(button_state) => Container::new(
                Button::new(button_state, Text::new("asdf"))
                    .on_press(Message::MoveScene(Scene::ArchiveView)),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(3)
            .style(style::Dark)
            .into(),
            Content::ArchiveView(content) => content.view(),
            Content::SchemeView(content) => Container::new(Text::new("asdf"))
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(3)
                .style(style::Dark)
                .into(),
        }
    }
}
