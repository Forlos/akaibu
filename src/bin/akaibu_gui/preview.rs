use crate::{message::Message, style};
use akaibu::resource;
use iced::{
    image, Column, Container, Element, HorizontalAlignment, Image, Length,
    Text, VerticalAlignment,
};

pub(crate) struct Preview {
    pub(crate) resource: resource::ResourceType,
}

impl Preview {
    pub(crate) fn new() -> Self {
        Self {
            resource: resource::ResourceType::Other,
        }
    }
    pub(crate) fn view(&mut self) -> Element<Message> {
        let mut content = Column::new();

        match &self.resource {
            resource::ResourceType::RgbaImage {
                pixels,
                width,
                height,
            } => {
                content = content.push(
                    Container::new(Image::new(image::Handle::from_pixels(
                        *width,
                        *height,
                        pixels.to_vec(),
                    )))
                    .center_x()
                    .center_y()
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(style::Dark),
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
                    .style(style::Dark),
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
                    .style(style::Dark),
                )
            }
        };

        Container::new(content)
            .height(Length::Fill)
            .width(Length::FillPortion(2))
            .into()
    }
}
