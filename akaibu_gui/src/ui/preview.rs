use crate::{message::Message, style};
use akaibu::resource;
use iced::{
    Column, Container, Element, HorizontalAlignment, Image, Length, Text,
    VerticalAlignment,
};
use image::{buffer::ConvertBuffer, ImageBuffer};

pub struct Preview {
    resource: resource::ResourceType,
    is_visible: bool,
}

impl Preview {
    pub fn new() -> Self {
        Self {
            resource: resource::ResourceType::Other,
            is_visible: false,
        }
    }
    pub fn view(&mut self) -> Element<Message> {
        let mut content = Column::new();

        match &self.resource {
            resource::ResourceType::RgbaImage { image } => {
                let bgra: ImageBuffer<image::Bgra<u8>, Vec<u8>> =
                    image.convert();
                content = content.push(
                    Container::new(Image::new(
                        iced::image::Handle::from_pixels(
                            bgra.width(),
                            bgra.height(),
                            bgra.into_vec(),
                        ),
                    ))
                    .center_x()
                    .center_y()
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(style::Dark::default()),
                )
            }
            resource::ResourceType::Text(text) => {
                content = content.push(
                    Container::new(
                        Text::new(text)
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .vertical_alignment(VerticalAlignment::Center)
                            .horizontal_alignment(HorizontalAlignment::Center),
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(style::Dark::default()),
                )
            }
            resource::ResourceType::Other => {
                content = content.push(
                    Container::new(
                        Text::new("No preview available...")
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .vertical_alignment(VerticalAlignment::Center)
                            .horizontal_alignment(HorizontalAlignment::Center),
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(style::Dark::default()),
                )
            }
        };

        Container::new(content)
            .height(Length::Fill)
            .width(Length::FillPortion(2))
            .into()
    }
    pub fn set_visible(&mut self, visible: bool) {
        self.is_visible = visible;
    }
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }
    pub fn set_resource(&mut self, resource: resource::ResourceType) {
        self.resource = resource;
    }
}
