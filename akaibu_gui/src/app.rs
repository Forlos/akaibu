use crate::{
    message::Message,
    ui::{archive::ArchiveContent, content::Content, scheme::SchemeContent},
    update, Opt,
};
use akaibu::magic;
use iced::{executor, Application, Command};
use std::{fs::File, io::Read};
use structopt::StructOpt;

pub(crate) struct App {
    pub(crate) opt: Opt,
    pub(crate) content: Content,
}

impl Application for App {
    type Message = Message;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        let opt = Opt::from_args();

        let mut magic = vec![0; 32];
        File::open(&opt.file)
            .expect("Could not open file")
            .read_exact(&mut magic)
            .expect("Could not read file");
        let archive_magic = magic::Archive::parse(&magic);

        let schemes = archive_magic.get_schemes();

        if archive_magic.is_universal() {
            let scheme = schemes.get(0).expect("Expected universal scheme");
            let (archive, dir) =
                scheme.extract(&opt.file).expect("Could not extract");
            (
                Self {
                    opt,
                    content: Content::ArchiveView(Box::new(
                        ArchiveContent::new(archive, dir),
                    )),
                },
                Command::none(),
            )
        } else {
            (
                Self {
                    opt,
                    content: Content::SchemeView(SchemeContent::new(schemes)),
                },
                Command::none(),
            )
        }
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
