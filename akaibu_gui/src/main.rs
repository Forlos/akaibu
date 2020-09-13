mod app;
mod logic;
mod message;
mod style;
mod ui;
mod update;

use app::App;
use iced::{window, Application, Settings};
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt()]
pub(crate) struct Opt {
    /// File to process
    #[structopt(required = true, name = "ARCHIVE", parse(from_os_str))]
    pub(crate) file: PathBuf,
}

fn main() {
    env_logger::init();

    App::run(Settings {
        // TODO this is workaround until iced supports fallback fonts
        // See: https://github.com/hecrj/iced/issues/33
        default_font: Some(include_bytes!(
            "../fonts/RictyDiminished-with-FiraCode-Regular.ttf"
        )),
        antialiasing: true,
        window: window::Settings {
            size: (1280, 720),
            ..Default::default()
        },
        ..Default::default()
    })
}

use rust_embed::RustEmbed;

#[derive(Debug, RustEmbed)]
#[folder = "resources/"]
pub struct Resources;
