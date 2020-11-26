use crate::{message::Message, style};
use akaibu::resource;
use iced::{
    button, Button, Column, Container, Element, HorizontalAlignment, Image,
    Length, Row, Space, Text, VerticalAlignment,
};
use image::{buffer::ConvertBuffer, ImageBuffer};

pub struct Preview {
    resource: resource::ResourceType,
    is_visible: bool,
    file_name: String,
    close_button_state: button::State,
}

impl Preview {
    pub fn new() -> Self {
        Self {
            resource: resource::ResourceType::Other,
            is_visible: false,
            file_name: String::new(),
            close_button_state: button::State::new(),
        }
    }
    pub fn view(&mut self) -> Element<'_, Message> {
        let x_image = iced::image::Handle::from_memory(
            crate::Resources::get("icons/x.png")
                .expect("Could not embedded resource")
                .into(),
        );
        let mut header = Row::new()
            .push(Space::new(Length::Units(5), Length::Units(0)))
            .push(Text::new(&self.file_name));
        let preview = match &self.resource {
            resource::ResourceType::RgbaImage { image } => {
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
            resource::ResourceType::Text(text) => Container::new(
                Text::new(text)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .vertical_alignment(VerticalAlignment::Center)
                    .horizontal_alignment(HorizontalAlignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill),
            resource::ResourceType::Other => Container::new(
                Text::new("No preview available...")
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .vertical_alignment(VerticalAlignment::Center)
                    .horizontal_alignment(HorizontalAlignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill),
        };
        header = header
            .push(Space::new(Length::Fill, Length::Units(0)))
            .push(
                Button::new(
                    &mut self.close_button_state,
                    iced::image::Image::new(x_image),
                )
                .style(style::Dark::default())
                .on_press(Message::ClosePreview),
            );

        Container::new(Column::new().push(header).push(preview))
            .height(Length::Fill)
            .width(Length::Fill)
            .style(style::Dark::default())
            .into()
    }
    pub fn set_visible(&mut self, visible: bool) {
        self.is_visible = visible;
    }
    pub fn is_visible(&self) -> bool {
        self.is_visible
    }
    pub fn set_resource(
        &mut self,
        resource: resource::ResourceType,
        file_name: String,
    ) {
        self.resource = resource;
        self.file_name = file_name;
    }
}
