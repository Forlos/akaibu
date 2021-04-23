use crate::{
    message::{Message, Scene, Status},
    style,
    ui::footer::Footer,
};
use akaibu::resource::ResourceScheme;
use iced::{
    button, scrollable, Button, Column, Container, Element, Length, Row,
    Scrollable, Text,
};
use std::path::PathBuf;

pub struct ResourceSchemeContent {
    schemes: Vec<(Box<dyn ResourceScheme>, button::State)>,
    scrollable_state: scrollable::State,
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
            scrollable_state: scrollable::State::new(),
            message,
            footer,
            file_path,
        }
    }
    pub fn view(&mut self) -> Element<'_, Message> {
        let file_path = self.file_path.clone();
        let schemes =
            Container::new(Scrollable::new(&mut self.scrollable_state).push(
                self.schemes.iter_mut().fold(
                    Column::new().spacing(5),
                    |col, (scheme, button_state)| {
                        col.push(
                            Row::new().push(
                                Button::new(
                                    button_state,
                                    Text::new(scheme.get_name()),
                                )
                                .on_press(Message::MoveScene(
                                    Scene::ResourceView(
                                        scheme.clone(),
                                        file_path.clone(),
                                    ),
                                ))
                                .style(style::Dark::default()),
                            ),
                        )
                    },
                ),
            ))
            .center_x()
            .center_y()
            .width(Length::Fill)
            .height(Length::Fill)
            .style(style::Dark {
                border_width: 0.0,
                ..Default::default()
            });
        let header = Container::new(Text::new(&self.message).size(30))
            .center_x()
            .center_y()
            .width(Length::Fill)
            .height(Length::Units(40))
            .style(style::Dark {
                border_width: 0.0,
                ..Default::default()
            });
        Column::new()
            .push(header)
            .push(schemes)
            .push(self.footer.view())
            .into()
    }
    pub fn set_status(&mut self, status: Status) {
        self.footer.set_status(status);
    }
}
