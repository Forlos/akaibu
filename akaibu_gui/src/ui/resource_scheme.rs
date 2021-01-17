use crate::{
    message::{Message, Scene, Status},
    style,
    ui::footer::Footer,
};
use akaibu::resource::ResourceScheme;
use iced::{button, Button, Column, Container, Element, Length, Row, Text};
use std::path::PathBuf;

pub struct ResourceSchemeContent {
    schemes: Vec<(Box<dyn ResourceScheme>, button::State)>,
    message: String,
    footer: Footer,
    file_path: PathBuf,
}

impl ResourceSchemeContent {
    pub fn new(
        schemes: Vec<Box<dyn ResourceScheme>>,
        message: String,
        file_path: PathBuf,
    ) -> Self {
        let schemes = schemes
            .into_iter()
            .map(|scheme| (scheme, button::State::new()))
            .collect();
        let footer = Footer::new();
        Self {
            schemes,
            message,
            footer,
            file_path,
        }
    }
    pub fn view(&mut self) -> Element<'_, Message> {
        let file_path = self.file_path.clone();
        let schemes = Container::new(
            self.schemes.iter_mut().fold(
                Column::new()
                    .spacing(5)
                    .push(Text::new(&self.message).size(30)),
                |col, (scheme, button_state)| {
                    col.push(
                        Row::new().push(
                            Button::new(
                                button_state,
                                Text::new(scheme.get_name()),
                            )
                            .on_press(Message::MoveScene(Scene::ResourceView(
                                scheme.clone(),
                                file_path.clone(),
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
