use iced::{
    button, checkbox, container, pick_list, progress_bar, text_input,
    Background, Color, Vector,
};

pub const DARK: Color = Color::from_rgb(
    0x19 as f32 / 255.0,
    0x1B as f32 / 255.0,
    0x28 as f32 / 255.0,
);

pub const DARK_FOCUSED: Color = Color::from_rgb(
    0x29 as f32 / 255.0,
    0x2B as f32 / 255.0,
    0x38 as f32 / 255.0,
);

pub const DARK_BUTTON_FOCUSED: Color = Color::from_rgb(
    0x2C as f32 / 255.0,
    0x2F as f32 / 255.0,
    0x3B as f32 / 255.0,
);

pub const DARK_SELECTION: Color = Color::from_rgb(
    0x82 as f32 / 255.0,
    0xAA as f32 / 255.0,
    0xFF as f32 / 255.0,
);

pub const TEXT_COLOR: Color = Color::from_rgb(
    0x82 as f32 / 255.0,
    0x8B as f32 / 255.0,
    0xB8 as f32 / 255.0,
);

pub const BORDER_COLOR: Color = Color::from_rgb(
    0x13 as f32 / 255.0,
    0x14 as f32 / 255.0,
    0x21 as f32 / 255.0,
);

pub struct Dark {
    pub border_width: f32,
    pub background: Background,
}

impl Default for Dark {
    fn default() -> Self {
        Self {
            border_width: 1.0,
            background: Background::Color(DARK),
        }
    }
}

impl container::StyleSheet for Dark {
    fn style(&self) -> container::Style {
        container::Style {
            background: Some(self.background),
            border_radius: 0.0,
            border_width: self.border_width,
            border_color: BORDER_COLOR,
            text_color: Some(TEXT_COLOR),
        }
    }
}
impl button::StyleSheet for Dark {
    fn active(&self) -> button::Style {
        button::Style {
            shadow_offset: Vector::new(0.0, 0.0),
            background: Some(Background::Color(DARK_FOCUSED)),
            border_radius: 0.0,
            border_width: self.border_width,
            border_color: BORDER_COLOR,
            text_color: TEXT_COLOR,
        }
    }
    fn hovered(&self) -> button::Style {
        let active = self.active();

        button::Style {
            shadow_offset: active.shadow_offset + Vector::new(0.0, 1.0),
            background: Some(Background::Color(DARK_BUTTON_FOCUSED)),
            ..active
        }
    }
    fn pressed(&self) -> button::Style {
        button::Style {
            shadow_offset: Vector::default(),
            ..self.active()
        }
    }
    fn disabled(&self) -> button::Style {
        let active = self.active();

        button::Style {
            shadow_offset: Vector::default(),
            background: active.background.map(|background| match background {
                Background::Color(color) => Background::Color(Color {
                    a: color.a * 0.1,
                    ..color
                }),
            }),
            text_color: Color {
                a: active.text_color.a * 0.1,
                ..active.text_color
            },
            ..active
        }
    }
}

impl text_input::StyleSheet for Dark {
    fn active(&self) -> text_input::Style {
        text_input::Style {
            background: Background::Color(HEADER),
            border_radius: 0.0,
            border_width: self.border_width,
            border_color: BORDER_COLOR,
        }
    }
    fn focused(&self) -> text_input::Style {
        text_input::Style {
            background: Background::Color(DARK_FOCUSED),
            border_radius: 0.0,
            border_width: self.border_width,
            border_color: BORDER_COLOR,
        }
    }
    fn placeholder_color(&self) -> Color {
        Color {
            a: 0.1,
            ..TEXT_COLOR
        }
    }
    fn value_color(&self) -> Color {
        TEXT_COLOR
    }
    fn selection_color(&self) -> Color {
        DARK_SELECTION
    }
    fn hovered(&self) -> text_input::Style {
        self.focused()
    }
}

impl progress_bar::StyleSheet for Dark {
    fn style(&self) -> progress_bar::Style {
        progress_bar::Style {
            background: self.background,
            bar: Background::Color(TEXT_COLOR),
            border_radius: 0.0,
        }
    }
}

impl checkbox::StyleSheet for Dark {
    fn active(&self, _is_checked: bool) -> checkbox::Style {
        checkbox::Style {
            background: self.background,
            checkmark_color: TEXT_COLOR,
            border_radius: 0.0,
            border_width: self.border_width,
            border_color: Color::BLACK,
        }
    }

    fn hovered(&self, _is_checked: bool) -> checkbox::Style {
        checkbox::Style {
            background: Background::Color(DARK_FOCUSED),
            checkmark_color: TEXT_COLOR,
            border_radius: 0.0,
            border_width: self.border_width,
            border_color: Color::BLACK,
        }
    }
}

impl pick_list::StyleSheet for Dark {
    fn menu(&self) -> pick_list::Menu {
        pick_list::Menu {
            background: self.background,
            border_width: self.border_width,
            border_color: BORDER_COLOR,
            text_color: TEXT_COLOR,
            selected_text_color: Color::BLACK,
            selected_background: Background::Color(DARK_SELECTION),
        }
    }

    fn active(&self) -> pick_list::Style {
        pick_list::Style {
            background: self.background,
            border_radius: 0.0,
            border_width: self.border_width,
            border_color: Color::BLACK,
            text_color: TEXT_COLOR,
            icon_size: 0.0,
        }
    }

    fn hovered(&self) -> pick_list::Style {
        pick_list::Style {
            background: Background::Color(DARK_FOCUSED),
            border_radius: 0.0,
            border_width: self.border_width,
            border_color: Color::BLACK,
            text_color: TEXT_COLOR,
            icon_size: 0.0,
        }
    }
}

pub const HEADER: Color = Color::from_rgb(
    0x1B as f32 / 255.0,
    0x1D as f32 / 255.0,
    0x2C as f32 / 255.0,
);

pub const HEADER_TEXT_HOVER: Color = Color::from_rgb(
    0x96 as f32 / 255.0,
    0x9F as f32 / 255.0,
    0xCB as f32 / 255.0,
);

pub const HEADER_TEXT_PRESSED: Color = Color::from_rgb(
    0x96 as f32 / 255.0,
    0x9F as f32 / 255.0,
    0xCB as f32 / 255.0,
);

pub(crate) struct Header;
impl container::StyleSheet for Header {
    fn style(&self) -> container::Style {
        container::Style {
            background: Some(Background::Color(HEADER)),
            border_width: 1.0,
            border_color: BORDER_COLOR,
            text_color: Some(Color { ..TEXT_COLOR }),
            border_radius: 0.0,
        }
    }
}
impl button::StyleSheet for Header {
    fn active(&self) -> button::Style {
        button::Style {
            shadow_offset: Vector::new(0.0, 0.0),
            background: Some(Background::Color(HEADER)),
            border_radius: 0.0,
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
            text_color: TEXT_COLOR,
        }
    }
    fn hovered(&self) -> button::Style {
        button::Style {
            text_color: HEADER_TEXT_HOVER,
            ..self.active()
        }
    }
    fn pressed(&self) -> button::Style {
        button::Style {
            text_color: HEADER_TEXT_PRESSED,
            ..self.active()
        }
    }
    fn disabled(&self) -> button::Style {
        let active = self.active();

        button::Style {
            shadow_offset: Vector::default(),
            background: active.background.map(|background| match background {
                Background::Color(color) => Background::Color(Color {
                    a: color.a * 0.1,
                    ..color
                }),
            }),
            text_color: Color {
                a: active.text_color.a * 0.1,
                ..active.text_color
            },
            ..active
        }
    }
}

pub const ERROR_TEXT_COLOR: Color = Color::from_rgb(
    0x80 as f32 / 255.0,
    0x20 as f32 / 255.0,
    0x20 as f32 / 255.0,
);

pub(crate) struct Error;
impl container::StyleSheet for Error {
    fn style(&self) -> container::Style {
        container::Style {
            background: Some(Background::Color(DARK)),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
            text_color: Some(ERROR_TEXT_COLOR),
            border_radius: 0.0,
        }
    }
}

pub const SUCCESS_TEXT_COLOR: Color = Color::from_rgb(
    0x20 as f32 / 255.0,
    0x80 as f32 / 255.0,
    0x20 as f32 / 255.0,
);

pub(crate) struct Success;
impl container::StyleSheet for Success {
    fn style(&self) -> container::Style {
        container::Style {
            background: Some(Background::Color(DARK)),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
            text_color: Some(SUCCESS_TEXT_COLOR),
            border_radius: 0.0,
        }
    }
}

pub const LIST_TEXT_HOVER: Color = Color::from_rgb(
    0xA9 as f32 / 255.0,
    0xB2 as f32 / 255.0,
    0xDF as f32 / 255.0,
);

pub const LIST_TEXT_PRESSED: Color = Color::from_rgb(
    0xA9 as f32 / 255.0,
    0xB2 as f32 / 255.0,
    0xDF as f32 / 255.0,
);

pub(crate) struct List;
impl button::StyleSheet for List {
    fn active(&self) -> button::Style {
        button::Style {
            shadow_offset: Vector::new(0.0, 0.0),
            background: Some(Background::Color(DARK)),
            border_radius: 0.0,
            border_width: 0.0,
            border_color: Color::BLACK,
            text_color: TEXT_COLOR,
        }
    }
    fn hovered(&self) -> button::Style {
        button::Style {
            text_color: LIST_TEXT_HOVER,
            ..self.active()
        }
    }
    fn pressed(&self) -> button::Style {
        button::Style {
            text_color: LIST_TEXT_PRESSED,
            ..self.active()
        }
    }
    fn disabled(&self) -> button::Style {
        let active = self.active();

        button::Style {
            shadow_offset: Vector::default(),
            background: active.background.map(|background| match background {
                Background::Color(color) => Background::Color(Color {
                    a: color.a * 0.1,
                    ..color
                }),
            }),
            text_color: Color {
                a: active.text_color.a * 0.1,
                ..active.text_color
            },
            ..active
        }
    }
}
