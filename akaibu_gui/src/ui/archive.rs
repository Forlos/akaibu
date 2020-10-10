use crate::{
    message::Message, message::Status, style, ui::footer::Footer,
    ui::preview::Preview,
};
use akaibu::archive;
use anyhow::Context;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use iced::{
    button, image, scrollable, text_input, Button, Checkbox, Column, Container,
    Element, Image, Length, Row, Scrollable, Space, Text, TextInput,
};
use itertools::Itertools;
use std::sync::Arc;

pub struct ArchiveContent {
    entries: Vec<Entry>,
    pub archive: Arc<Box<dyn archive::Archive>>,
    pub navigable_dir: archive::NavigableDirectory,
    entries_scrollable_state: scrollable::State,
    extract_all_button_state: button::State,
    pub convert_all: bool,
    back_dir_button_state: button::State,
    pub preview: Preview,
    footer: Footer,
    pattern_text_input: text_input::State,
    fuzzy_matcher: SkimMatcherV2,
    pub pattern: String,
}

impl ArchiveContent {
    pub fn new(
        archive: Box<dyn archive::Archive>,
        navigable_dir: archive::NavigableDirectory,
    ) -> Self {
        let current = navigable_dir.get_current();
        let entries = Self::new_entries(current);
        let footer = Footer::new();
        Self {
            entries,
            archive: Arc::new(archive),
            navigable_dir,
            entries_scrollable_state: scrollable::State::new(),
            extract_all_button_state: button::State::new(),
            convert_all: false,
            back_dir_button_state: button::State::new(),
            preview: Preview::new(),
            footer,
            pattern_text_input: text_input::State::new(),
            fuzzy_matcher: SkimMatcherV2::default(),
            pattern: String::new(),
        }
    }
    pub fn view(&mut self) -> Element<Message> {
        let mut column = Column::new()
            .push(
                Column::new()
                    .height(Length::FillPortion(2))
                    .push(Space::new(Length::Units(0), Length::Units(5)))
                    .push(
                        Row::new()
                            .height(Length::Units(30))
                            .spacing(5)
                            .push(Space::new(
                                Length::Units(0),
                                Length::Units(0),
                            ))
                            .push(
                                Button::new(
                                    &mut self.extract_all_button_state,
                                    Text::new("Extract all"),
                                )
                                .on_press(Message::ExtractAll)
                                .style(style::Dark::default()),
                            )
                            .push(
                                Container::new(
                                    Checkbox::new(
                                        self.convert_all,
                                        "Convert all",
                                        Message::ConvertAllToggle,
                                    )
                                    .text_size(16)
                                    .spacing(3)
                                    .style(style::Dark::default()),
                                )
                                .height(Length::Fill)
                                .center_y()
                                .center_x(),
                            )
                            .push({
                                let back_button = Button::new(
                                    &mut self.back_dir_button_state,
                                    Text::new("Back dir"),
                                )
                                .style(style::Dark::default());
                                if self.navigable_dir.has_parent() {
                                    back_button.on_press(Message::BackDirectory)
                                } else {
                                    back_button
                                }
                            })
                            .push(
                                TextInput::new(
                                    &mut self.pattern_text_input,
                                    "Search...",
                                    &self.pattern,
                                    Message::PatternChanged,
                                )
                                .style(style::Dark::default()),
                            )
                            .push(Space::new(
                                Length::Units(0),
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
                            .push({
                                let matcher = &self.fuzzy_matcher;
                                let pattern = &self.pattern;
                                self.entries
                                    .iter_mut()
                                    .filter(|entry| {
                                        matcher
                                            .fuzzy_match(
                                                entry.get_name(),
                                                pattern,
                                            )
                                            .is_some()
                                    })
                                    .fold(Column::new(), |col, entry| {
                                        col.push(entry.view())
                                    })
                            }),
                    ),
            )
            .height(Length::Fill);
        if self.preview.is_visible() {
            column = column.push(
                Container::new(self.preview.view())
                    .height(Length::FillPortion(3)),
            );
        }
        let content = Column::new().push(column).push(self.footer.view());
        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(style::Dark::default())
            .into()
    }
    pub fn move_dir(&mut self, dir_name: String) -> anyhow::Result<()> {
        self.entries = Self::new_entries(
            self.navigable_dir
                .move_dir(&dir_name)
                .context("Could not move into directory")?,
        );
        self.footer
            .set_current_dir(self.navigable_dir.get_current_full_path());
        self.pattern = String::new();
        Ok(())
    }
    pub fn back_dir(&mut self) -> anyhow::Result<()> {
        self.entries = Self::new_entries(
            self.navigable_dir
                .back_dir()
                .context("Could not move back directory")?,
        );
        self.footer
            .set_current_dir(self.navigable_dir.get_current_full_path());
        self.pattern = String::new();
        Ok(())
    }
    pub fn set_status(&mut self, status: Status) {
        self.footer.set_status(status);
    }
    pub fn set_progress(&mut self, progress: f32) {
        self.footer.set_progress(progress);
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
    fn get_name(&self) -> &str {
        match self {
            Entry::Directory { dir_name, .. } => dir_name,
            Entry::File { file, .. } => &file.file_name,
        }
    }
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
                                .push(Space::new(
                                    Length::Units(5),
                                    Length::Units(0),
                                ))
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
                        .style(style::Dark::default()),
                    )
                    .push(
                        Container::new(
                            Text::new(format!("{}", file_count)).size(16),
                        )
                        .width(Length::Units(80))
                        .height(Length::Fill)
                        .center_y()
                        .padding(5)
                        .style(style::Dark::default()),
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
                            .width(Length::Units(65))
                            .height(Length::Units(25))
                            .style(style::Dark::default()),
                        )
                        .center_y()
                        .center_x()
                        .width(Length::Units(210))
                        .height(Length::Fill)
                        .style(style::Dark::default()),
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
                                .push(Space::new(
                                    Length::Units(5),
                                    Length::Units(0),
                                ))
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
                        .style(style::Dark::default()),
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
                        .style(style::Dark::default()),
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
                            .width(Length::Units(65))
                            .height(Length::Units(25))
                            .style(style::Dark::default()),
                        )
                        .center_y()
                        .center_x()
                        .width(Length::Units(70))
                        .height(Length::Fill)
                        .style(style::Dark::default()),
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
                            .width(Length::Units(65))
                            .height(Length::Units(25))
                            .style(style::Dark::default()),
                        )
                        .center_y()
                        .center_x()
                        .width(Length::Units(70))
                        .height(Length::Fill)
                        .style(style::Dark::default()),
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
                            .width(Length::Units(65))
                            .height(Length::Units(25))
                            .style(style::Dark::default()),
                        )
                        .center_y()
                        .center_x()
                        .width(Length::Units(70))
                        .height(Length::Fill)
                        .style(style::Dark::default()),
                    )
                    .push(Space::new(Length::Units(5), Length::Units(0)))
                    .height(Length::Units(30));
                Container::new(content).into()
            }
        }
    }
}
