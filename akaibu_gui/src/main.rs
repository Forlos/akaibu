mod app;
mod content;
mod message;
mod preview;
mod style;
mod update;

use app::App;
use iced::{Application, Settings};
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt()]
pub(crate) struct Opt {
    /// Files to process
    #[structopt(required = true, name = "ARCHIVES", parse(from_os_str))]
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
        ..Default::default()
    })
}
