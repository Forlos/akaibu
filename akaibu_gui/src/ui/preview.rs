use crate::{message::Message, style};
use akaibu::resource::{self, ResourceType};
use iced::{
    button,
    image::{viewer, Viewer},
    Button, Column, Container, Element, HorizontalAlignment, Image, Length,
    Row, Space, Text, VerticalAlignment,
};
use image::{buffer::ConvertBuffer, ImageBuffer};
use once_cell::sync::Lazy;

static X_IMAGE_HANDLE: Lazy<iced::image::Handle> = Lazy::new(|| {
    iced::image::Handle::from_memory(
        crate::Resources::get("icons/x.png")
            .expect("Could not embedded resource")
            .into(),
    )
});

pub struct Preview {
    resource: resource::ResourceType,
    is_visible: bool,
    file_name: String,
    close_button_state: button::State,
    prev_sprite_button_state: button::State,
    next_sprite_button_state: button::State,
    image_viewer_state: viewer::State,
    sprite_index: usize,
}

impl Preview {
    pub fn new() -> Self {
        Self {
            resource: resource::ResourceType::Other,
            is_visible: false,
            file_name: String::new(),
            close_button_state: button::State::new(),
            prev_sprite_button_state: button::State::new(),
            next_sprite_button_state: button::State::new(),
            image_viewer_state: viewer::State::new(),
            sprite_index: 0,
        }
    }
    pub fn view(&mut self) -> Element<'_, Message> {
        let mut header = Row::new()
            .push(Space::new(Length::Units(5), Length::Units(0)))
            .push(Text::new(&self.file_name));
        let preview = match &self.resource {
            resource::ResourceType::SpriteSheet { sprites } => {
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
                Container::new(Viewer::new(
                    &mut self.image_viewer_state,
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
            }
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
                Container::new(Viewer::new(
                    &mut self.image_viewer_state,
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
        header = header.push(Space::new(Length::Fill, Length::Units(0)));
        if let ResourceType::SpriteSheet { sprites } = &self.resource {
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
                .push(prev)
                .push(Space::new(Length::Units(5), Length::Units(0)))
                .push(next)
                .push(Space::new(Length::Units(5), Length::Units(0)));
        }
        header = header.push(
            Button::new(
                &mut self.close_button_state,
                Image::new(X_IMAGE_HANDLE.clone()),
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
        self.sprite_index = 0;
    }
    pub fn inc_sprite_index(&mut self) {
        self.sprite_index += 1;
    }
    pub fn dec_sprite_index(&mut self) {
        self.sprite_index -= 1;
    }
}
