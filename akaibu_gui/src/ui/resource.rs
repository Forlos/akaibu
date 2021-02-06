use super::footer::Footer;
use crate::{
    message::{Message, Status},
    style,
};
use akaibu::resource::ResourceType;
use iced::{
    button, pick_list, Button, Column, Container, Element, HorizontalAlignment,
    Image, Length, PickList, Row, Space, Text, VerticalAlignment,
};
use image::{buffer::ConvertBuffer, ImageBuffer};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertFormat {
    PNG,
    JPEG,
    BMP,
    TIFF,
    ICO,
}

impl ConvertFormat {
    const ALL: [ConvertFormat; 5] =
        [Self::PNG, Self::JPEG, Self::BMP, Self::TIFF, Self::ICO];
}

impl std::fmt::Display for ConvertFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::PNG => "PNG",
                Self::JPEG => "JPEG",
                Self::BMP => "BMP",
                Self::TIFF => "TIFF",
                Self::ICO => "ICO",
            }
        )
    }
}

pub struct ResourceContent {
    pub file_name: PathBuf,
    pub resource: ResourceType,
    footer: Footer,
    format_list: pick_list::State<ConvertFormat>,
    pub format: ConvertFormat,
    convert_button_state: button::State,
    prev_sprite_button_state: button::State,
    next_sprite_button_state: button::State,
    sprite_index: usize,
}

impl ResourceContent {
    pub fn new(resource: ResourceType, file_name: PathBuf) -> Self {
        let mut footer = Footer::new();
        footer.set_current_dir(format!("{:?}", file_name));
        let format_list = pick_list::State::default();
        let format = ConvertFormat::PNG;
        let convert_button_state = button::State::new();
        Self {
            file_name,
            resource,
            footer,
            format_list,
            format,
            convert_button_state,
            prev_sprite_button_state: button::State::new(),
            next_sprite_button_state: button::State::new(),
            sprite_index: 0,
        }
    }
    pub fn view(&mut self) -> Element<'_, Message> {
        let mut header =
            Row::new().push(Space::new(Length::Units(5), Length::Units(0)));
        let resource = match &self.resource {
            ResourceType::SpriteSheet { sprites } => {
                let bgra: ImageBuffer<image::Bgra<u8>, Vec<u8>> = sprites
                    .get(self.sprite_index)
                    .expect("Could not get sprite")
                    .convert();
                header = header
                    .push(Space::new(Length::Units(5), Length::Units(0)))
                    .push(Text::new(format!(
                        "Sprite {}x{}px",
                        bgra.width(),
                        bgra.height()
                    )));
                Container::new(Image::new(iced::image::Handle::from_pixels(
                    bgra.width(),
                    bgra.height(),
                    bgra.into_vec(),
                )))
                .center_x()
                .center_y()
                .width(Length::Fill)
                .height(Length::Fill)
            }
            ResourceType::RgbaImage { image } => {
                let bgra: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
                    image.convert();
                header = header
                    .push(Space::new(Length::Units(5), Length::Units(0)))
                    .push(Text::new(format!(
                        "Image {}x{}px",
                        bgra.width(),
                        bgra.height()
                    )));
                Container::new(Image::new(iced::image::Handle::from_pixels(
                    bgra.width(),
                    bgra.height(),
                    bgra.into_vec(),
                )))
                .center_x()
                .center_y()
                .width(Length::Fill)
                .height(Length::Fill)
            }
            ResourceType::Text(text) => Container::new(
                Text::new(text)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .vertical_alignment(VerticalAlignment::Center)
                    .horizontal_alignment(HorizontalAlignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill),
            ResourceType::Other => Container::new(
                Text::new("No preview available...")
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .vertical_alignment(VerticalAlignment::Center)
                    .horizontal_alignment(HorizontalAlignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill),
        };
        if let ResourceType::RgbaImage { image: _ } = &self.resource {
            header = header
                .push(Space::new(Length::Fill, Length::Units(0)))
                .push(
                    Button::new(
                        &mut self.convert_button_state,
                        Container::new(Text::new("Save as").size(16))
                            .center_x()
                            .center_y(),
                    )
                    .on_press(Message::SaveResource)
                    .style(style::Dark::default()),
                )
                .push(Space::new(Length::Units(5), Length::Units(0)))
                .push(
                    PickList::new(
                        &mut self.format_list,
                        &ConvertFormat::ALL[..],
                        Some(self.format),
                        Message::FormatChanged,
                    )
                    .style(style::Dark {
                        border_width: 0.0,
                        ..Default::default()
                    })
                    .text_size(16),
                )
                .push(Space::new(Length::Units(5), Length::Units(0)));
        } else if let ResourceType::SpriteSheet { sprites } = &self.resource {
            let mut prev = Button::new(
                &mut self.prev_sprite_button_state,
                Container::new(Text::new(" < ").size(16))
                    .center_x()
                    .center_y(),
            )
            .style(style::Dark::default());
            if self.sprite_index > 0 {
                prev = prev.on_press(Message::PrevSprite);
            }
            let mut next = Button::new(
                &mut self.next_sprite_button_state,
                Container::new(Text::new(" > ").size(16))
                    .center_x()
                    .center_y(),
            )
            .style(style::Dark::default());
            if self.sprite_index < sprites.len() - 1 {
                next = next.on_press(Message::NextSprite);
            }
            header = header
                .push(Space::new(Length::Fill, Length::Units(0)))
                .push(prev)
                .push(Space::new(Length::Units(5), Length::Units(0)))
                .push(next)
                .push(Space::new(Length::Units(5), Length::Units(0)))
                .push(
                    Button::new(
                        &mut self.convert_button_state,
                        Container::new(Text::new("Save as").size(16))
                            .center_x()
                            .center_y(),
                    )
                    .on_press(Message::SaveSprite(self.sprite_index))
                    .style(style::Dark::default()),
                )
                .push(Space::new(Length::Units(5), Length::Units(0)))
                .push(
                    PickList::new(
                        &mut self.format_list,
                        &ConvertFormat::ALL[..],
                        Some(self.format),
                        Message::FormatChanged,
                    )
                    .style(style::Dark {
                        border_width: 0.0,
                        ..Default::default()
                    })
                    .text_size(16),
                )
                .push(Space::new(Length::Units(5), Length::Units(0)));
        }
        let content = Container::new(resource)
            .center_x()
            .center_y()
            .width(Length::Fill)
            .height(Length::Fill)
            .style(style::Dark::default());
        Container::new(
            Column::new()
                .push(header)
                .push(content)
                .push(self.footer.view()),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(style::Dark::default())
        .into()
    }
    pub fn set_status(&mut self, status: Status) {
        self.footer.set_status(status);
    }
    pub fn inc_sprite_index(&mut self) {
        self.sprite_index += 1;
    }
    pub fn dec_sprite_index(&mut self) {
        self.sprite_index -= 1;
    }
}
