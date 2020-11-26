use crate::{
    message::{Message, Scene, Status},
    style,
    ui::footer::Footer,
};
use akaibu::scheme::Scheme;
use iced::{button, Button, Column, Container, Element, Length, Row, Text};

pub struct SchemeContent {
    schemes: Vec<(Box<dyn Scheme>, button::State)>,
    footer: Footer,
}

impl SchemeContent {
    pub fn new(schemes: Vec<Box<dyn Scheme>>) -> Self {
        let schemes = schemes
            .into_iter()
            .map(|scheme| (scheme, button::State::new()))
            .collect();
        let footer = Footer::new();
        Self { schemes, footer }
    }
    pub fn view(&mut self) -> Element<'_, Message> {
        let schemes = Container::new(
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
        .style(style::Dark::default());
        Column::new().push(schemes).push(self.footer.view()).into()
    }
    pub fn set_status(&mut self, status: Status) {
        self.footer.set_status(status);
    }
}
