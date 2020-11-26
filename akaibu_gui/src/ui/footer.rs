use crate::{message::Message, message::Status, style};
use iced::{
    Background, Container, Element, Length, ProgressBar, Row, Space, Text,
    VerticalAlignment,
};

pub struct Footer {
    current_dir: String,
    progress: f32,
    status: Status,
}

impl Footer {
    pub fn new() -> Self {
        Self {
            current_dir: String::from("/"),
            progress: 0.0,
            status: Status::Normal(String::new()),
        }
    }
    pub fn view(&mut self) -> Element<'_, Message> {
        let content = Row::new()
            .push(Space::new(Length::Units(5), Length::Units(0)))
            .push(
                Text::new(&self.current_dir)
                    .size(16)
                    .height(Length::Fill)
                    .vertical_alignment(VerticalAlignment::Center),
            )
            .push(Space::new(Length::Units(15), Length::Units(0)))
            .push(
                Container::new(
                    ProgressBar::new(0.0..=100.0, self.progress)
                        .height(Length::Units(10))
                        .style(style::Dark {
                            background: Background::Color(
                                style::DARK_BUTTON_FOCUSED,
                            ),
                            ..Default::default()
                        }),
                )
                .center_y()
                .height(Length::Fill)
                .width(Length::Fill),
            )
            .push(Space::new(Length::Units(15), Length::Units(0)))
            .push(match &self.status {
                Status::Normal(status) => Text::new(status)
                    .size(16)
                    .height(Length::Fill)
                    .vertical_alignment(VerticalAlignment::Center),
                Status::Error(status) => Text::new(status)
                    .color(style::ERROR_TEXT_COLOR)
                    .size(16)
                    .height(Length::Fill)
                    .vertical_alignment(VerticalAlignment::Center),
                Status::Success(status) => Text::new(status)
                    .color(style::SUCCESS_TEXT_COLOR)
                    .size(16)
                    .height(Length::Fill)
                    .vertical_alignment(VerticalAlignment::Center),
                Status::Empty => Text::new(""),
            })
            .push(Space::new(Length::Units(5), Length::Units(0)));
        Container::new(content)
            .height(Length::Units(20))
            .width(Length::Fill)
            .style(style::Dark {
                border_width: 0.0,
                background: Background::Color(style::DARK_BUTTON_FOCUSED),
            })
            .into()
    }
    pub fn set_current_dir(&mut self, new_dir: String) {
        self.current_dir = new_dir;
    }
    pub fn set_status(&mut self, status: Status) {
        self.status = status;
    }
    pub fn set_progress(&mut self, progress: f32) {
        self.progress = progress;
    }
}
