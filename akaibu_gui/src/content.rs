use crate::{message::Message, preview::Preview, style};
use iced::{
    button, scrollable, Button, Column, Container, Element, Length,
    ProgressBar, Row, Scrollable, Space, Text,
};

pub(crate) struct Content {
    pub(crate) entries: Vec<Entry>,
    entries_scrollable_state: scrollable::State,
    extract_all_button_state: button::State,
    pub(crate) preview: Preview,
    pub(crate) extract_all_progress: f32,
}

impl Content {
    pub(crate) fn new(entries: Vec<Entry>) -> Self {
        Self {
            entries,
            entries_scrollable_state: scrollable::State::new(),
            extract_all_button_state: button::State::new(),
            preview: Preview::new(),
            extract_all_progress: 0.0,
        }
    }
    pub(crate) fn view(&mut self) -> Element<Message> {
        let content = Row::new()
            .push(
                Column::new()
                    .width(Length::FillPortion(3))
                    .push(Space::new(Length::Units(0), Length::Units(5)))
                    .push(
                        Row::new()
                            .push(Space::new(
                                Length::Units(5),
                                Length::Units(0),
                            ))
                            .push(
                                Button::new(
                                    &mut self.extract_all_button_state,
                                    Text::new("Extract all"),
                                )
                                .on_press(Message::ExtractAll)
                                .style(style::Dark),
                            )
                            .push(Space::new(
                                Length::Units(5),
                                Length::Units(0),
                            ))
                            .push(
                                ProgressBar::new(
                                    0.0..=100.0,
                                    self.extract_all_progress,
                                )
                                .style(style::Dark),
                            )
                            .push(Space::new(
                                Length::Units(5),
                                Length::Units(0),
                            )),
                    )
                    .push(
                        Row::new()
                            .push(Space::new(
                                Length::Units(5),
                                Length::Units(0),
                            ))
                            .push(
                                Container::new(Text::new("Name"))
                                    .width(Length::FillPortion(1)),
                            )
                            .push(
                                Container::new(Text::new("Size"))
                                    .width(Length::Units(100)),
                            )
                            .push(
                                Container::new(Text::new("Extract"))
                                    .width(Length::Units(100)),
                            )
                            .push(
                                Container::new(Text::new("Preview"))
                                    .width(Length::Units(100)),
                            ),
                    )
                    .push(
                        Scrollable::new(&mut self.entries_scrollable_state)
                            .push(
                                self.entries
                                    .iter_mut()
                                    .fold(Column::new(), |col, entry| {
                                        col.push(entry.view())
                                    }),
                            ),
                    ),
            )
            .push(self.preview.view());
        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(3)
            .style(style::Dark)
            .into()
    }
}

pub(crate) struct Entry {
    pub(crate) file_entry: akaibu::archive::FileEntry,
    pub(crate) extract_button_state: button::State,
    pub(crate) preview_button_state: button::State,
}

impl Entry {
    pub fn new(file_entry: akaibu::archive::FileEntry) -> Self {
        Self {
            file_entry,
            extract_button_state: button::State::new(),
            preview_button_state: button::State::new(),
        }
    }
    fn view(&mut self) -> Element<Message> {
        let content = Row::new()
            .push(Space::new(Length::Units(5), Length::Units(0)))
            .push(
                Container::new(Text::new(&self.file_entry.file_name))
                    .width(Length::FillPortion(1))
                    .height(Length::Fill)
                    .center_y()
                    .padding(5)
                    .style(style::Dark),
            )
            .push(
                Container::new(Text::new(bytesize::to_string(
                    self.file_entry.file_size,
                    false,
                )))
                .width(Length::Units(100))
                .height(Length::Fill)
                .center_y()
                .padding(5)
                .style(style::Dark),
            )
            .push(
                Container::new(
                    Button::new(
                        &mut self.extract_button_state,
                        Container::new(Text::new("Extract"))
                            .center_y()
                            .center_x(),
                    )
                    .on_press(Message::ExtractFile(self.file_entry.clone()))
                    .padding(5)
                    .width(Length::Units(80))
                    .height(Length::Units(30))
                    .style(style::Dark),
                )
                .center_y()
                .center_x()
                .width(Length::Units(100))
                .height(Length::Fill)
                .style(style::Dark),
            )
            .push(
                Container::new(
                    Button::new(
                        &mut self.preview_button_state,
                        Container::new(Text::new("Preview"))
                            .center_y()
                            .center_x(),
                    )
                    .on_press(Message::PreviewFile(self.file_entry.clone()))
                    .padding(5)
                    .width(Length::Units(80))
                    .height(Length::Units(30))
                    .style(style::Dark),
                )
                .center_y()
                .center_x()
                .width(Length::Units(100))
                .height(Length::Fill)
                .style(style::Dark),
            )
            .push(Space::new(Length::Units(5), Length::Units(0)))
            .height(Length::Units(40));
        Container::new(content).into()
    }
}
