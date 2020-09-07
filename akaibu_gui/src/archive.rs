use crate::{message::Message, preview::Preview, style};
use akaibu::archive;
use iced::{
    button, image, scrollable, Button, Column, Container, Element, Image,
    Length, ProgressBar, Row, Scrollable, Space, Text,
};
use itertools::Itertools;

pub struct ArchiveContent {
    entries: Vec<Entry>,
    pub(crate) archive: Box<dyn archive::Archive>,
    entries_scrollable_state: scrollable::State,
    extract_all_button_state: button::State,
    back_dir_button_state: button::State,
    pub(crate) preview: Preview,
    pub(crate) extract_all_progress: f32,
}

impl ArchiveContent {
    pub fn new(mut archive: Box<dyn archive::Archive>) -> Self {
        let current = archive.get_navigable_dir().get_current();
        let entries = Self::new_entries(current);
        Self {
            entries,
            archive,
            entries_scrollable_state: scrollable::State::new(),
            extract_all_button_state: button::State::new(),
            back_dir_button_state: button::State::new(),
            preview: Preview::new(),
            extract_all_progress: 0.0,
        }
    }
    pub fn view(&mut self) -> Element<Message> {
        let mut back_button =
            Button::new(&mut self.back_dir_button_state, Text::new("Back dir"))
                .style(style::Dark);
        if self.archive.get_navigable_dir().has_parent() {
            back_button = back_button.on_press(Message::BackDirectory);
        }
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
                            .push(back_button)
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
                                Container::new(Text::new("Name").size(18))
                                    .width(Length::FillPortion(1)),
                            )
                            .push(
                                Container::new(Text::new("Size").size(18))
                                    .width(Length::Units(80)),
                            )
                            .push(
                                Container::new(Text::new("Actions").size(18))
                                    .width(Length::Units(210)),
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
    pub fn move_dir(&mut self, dir_name: String) {
        self.entries = Self::new_entries(
            self.archive
                .get_navigable_dir()
                .move_dir(&dir_name)
                .unwrap(),
        );
    }
    pub fn back_dir(&mut self) {
        self.entries = Self::new_entries(
            self.archive.get_navigable_dir().back_dir().unwrap(),
        );
    }
    fn new_entries(current: &archive::Directory) -> Vec<Entry> {
        current
            .directories
            .iter()
            .sorted_by(|(a, _), (b, _)| a.cmp(b))
            .map(|(name, dir)| Entry::Directory {
                dir_name: name.clone(),
                file_count: dir.files.len() + dir.directories.len(),
                open_button_state: button::State::new(),
            })
            .chain(current.files.iter().map(|f| Entry::File {
                file: f.clone(),
                convert_button_state: button::State::new(),
                extract_button_state: button::State::new(),
                preview_button_state: button::State::new(),
            }))
            .collect()
    }
}

enum Entry {
    Directory {
        dir_name: String,
        file_count: usize,
        open_button_state: button::State,
    },
    File {
        file: archive::FileEntry,
        convert_button_state: button::State,
        extract_button_state: button::State,
        preview_button_state: button::State,
    },
}

impl Entry {
    fn view(&mut self) -> Element<Message> {
        match self {
            Entry::Directory {
                dir_name,
                file_count,
                open_button_state,
            } => {
                let image_handle = image::Handle::from_memory(
                    crate::Resources::get("icons/folder.png")
                        .expect("Could not embedded resource")
                        .into(),
                );
                let content = Row::new()
                    .push(Space::new(Length::Units(5), Length::Units(0)))
                    .push(
                        Container::new(
                            Row::new()
                                .push(Image::new(image_handle))
                                .push(Space::new(
                                    Length::Units(5),
                                    Length::Units(0),
                                ))
                                .push(Text::new(&*dir_name).size(16)),
                        )
                        .width(Length::FillPortion(1))
                        .height(Length::Fill)
                        .center_y()
                        .padding(5)
                        .style(style::Dark),
                    )
                    .push(
                        Container::new(
                            Text::new(format!("{}", file_count)).size(16),
                        )
                        .width(Length::Units(80))
                        .height(Length::Fill)
                        .center_y()
                        .padding(5)
                        .style(style::Dark),
                    )
                    .push(
                        Container::new(
                            Button::new(
                                open_button_state,
                                Container::new(Text::new("Open").size(16))
                                    .center_y()
                                    .center_x(),
                            )
                            .on_press(Message::OpenDirectory(dir_name.clone()))
                            .padding(5)
                            .width(Length::Units(65))
                            .height(Length::Units(25))
                            .style(style::Dark),
                        )
                        .center_y()
                        .center_x()
                        .width(Length::Units(210))
                        .height(Length::Fill)
                        .style(style::Dark),
                    )
                    .push(Space::new(Length::Units(5), Length::Units(0)))
                    .height(Length::Units(30));
                Container::new(content).into()
            }
            Entry::File {
                file,
                convert_button_state,
                extract_button_state,
                preview_button_state,
            } => {
                let image_handle = image::Handle::from_memory(
                    crate::Resources::get("icons/file.png")
                        .expect("Could not get embedded resource")
                        .into(),
                );
                let content = Row::new()
                    .push(Space::new(Length::Units(5), Length::Units(0)))
                    .push(
                        Container::new(
                            Row::new()
                                .push(Image::new(image_handle))
                                .push(Space::new(
                                    Length::Units(5),
                                    Length::Units(0),
                                ))
                                .push(Text::new(&*file.file_name).size(16)),
                        )
                        .width(Length::FillPortion(1))
                        .height(Length::Fill)
                        .center_y()
                        .padding(5)
                        .style(style::Dark),
                    )
                    .push(
                        Container::new(
                            Text::new(bytesize::to_string(
                                file.file_size,
                                false,
                            ))
                            .size(16),
                        )
                        .width(Length::Units(80))
                        .height(Length::Fill)
                        .center_y()
                        .padding(5)
                        .style(style::Dark),
                    )
                    .push(
                        Container::new(
                            Button::new(
                                convert_button_state,
                                Container::new(Text::new("Convert").size(16))
                                    .center_y()
                                    .center_x(),
                            )
                            .on_press(Message::ConvertFile(file.clone()))
                            .padding(5)
                            .width(Length::Units(65))
                            .height(Length::Units(25))
                            .style(style::Dark),
                        )
                        .center_y()
                        .center_x()
                        .width(Length::Units(70))
                        .height(Length::Fill)
                        .style(style::Dark),
                    )
                    .push(
                        Container::new(
                            Button::new(
                                extract_button_state,
                                Container::new(Text::new("Extract").size(16))
                                    .center_y()
                                    .center_x(),
                            )
                            .on_press(Message::ExtractFile(file.clone()))
                            .padding(5)
                            .width(Length::Units(65))
                            .height(Length::Units(25))
                            .style(style::Dark),
                        )
                        .center_y()
                        .center_x()
                        .width(Length::Units(70))
                        .height(Length::Fill)
                        .style(style::Dark),
                    )
                    .push(
                        Container::new(
                            Button::new(
                                preview_button_state,
                                Container::new(Text::new("Preview").size(16))
                                    .center_y()
                                    .center_x(),
                            )
                            .on_press(Message::PreviewFile(file.clone()))
                            .padding(5)
                            .width(Length::Units(65))
                            .height(Length::Units(25))
                            .style(style::Dark),
                        )
                        .center_y()
                        .center_x()
                        .width(Length::Units(70))
                        .height(Length::Fill)
                        .style(style::Dark),
                    )
                    .push(Space::new(Length::Units(5), Length::Units(0)))
                    .height(Length::Units(30));
                Container::new(content).into()
            }
        }
    }
}
