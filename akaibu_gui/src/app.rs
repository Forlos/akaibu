use crate::{
    content::{Content, Entry},
    message::Message,
    update, Opt,
};
use akaibu::{archive, magic};
use iced::{executor, Application, Command};
use std::{fs::File, io::Read};
use structopt::StructOpt;

pub(crate) struct App {
    pub(crate) content: Content,
    pub(crate) archive: Box<dyn archive::Archive>,
}

impl Application for App {
    type Message = Message;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        let opt = Opt::from_args();
        let file = opt.file;

        let mut magic = vec![0; 32];
        File::open(&file)
            .expect("Could not open file")
            .read_exact(&mut magic)
            .expect("Could not read file");
        let archive_magic = magic::Archive::parse(&magic);

        let schemes = archive_magic.get_schemes();
        let scheme = if archive_magic.is_universal() {
            schemes.get(0).unwrap()
        } else {
            todo!()
        };
        let archive = scheme.extract(&file).unwrap();

        (
            Self {
                content: Content::new(
                    archive.get_files().into_iter().map(Entry::new).collect(),
                ),
                archive,
            },
            Command::none(),
        )
    }
    fn title(&self) -> String {
        "Akaibu".to_owned()
    }
    fn update(&mut self, message: Self::Message) -> Command<Message> {
        match update::handle_message(self, message) {
            Ok(command) => command,
            Err(err) => {
                log::error!("{:?}", err);
                Command::perform(async move { err.to_string() }, Message::Error)
            }
        }
    }
    fn view(&mut self) -> iced::Element<'_, Self::Message> {
        self.content.view()
    }
}
