use crate::{
    message::{Message, Scene},
    style,
};
use akaibu::scheme::Scheme;
use iced::{button, Button, Column, Container, Element, Length, Row, Text};

pub struct SchemeContent {
    schemes: Vec<(Box<dyn Scheme>, button::State)>,
}

impl SchemeContent {
    pub fn new(schemes: Vec<Box<dyn Scheme>>) -> Self {
        let schemes = schemes
            .into_iter()
            .map(|scheme| (scheme, button::State::new()))
            .collect();
        Self { schemes }
    }
    pub fn view(&mut self) -> Element<Message> {
        Container::new(
            self.schemes.iter_mut().fold(
                Column::new()
                    .spacing(5)
                    .push(Text::new("Select extract scheme").size(30)),
                |col, (scheme, button_state)| {
                    col.push(
                        Row::new().push(
                            Button::new(
                                button_state,
                                Text::new(scheme.get_name()),
                            )
                            .on_press(Message::MoveScene(Scene::ArchiveView(
                                scheme.clone(),
                            )))
                            .style(style::Dark::default()),
                        ),
                    )
                },
            ),
        )
        .center_x()
        .center_y()
        .width(Length::Fill)
        .height(Length::Fill)
        .style(style::Dark::default())
        .into()
    }
}
