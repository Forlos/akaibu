use crate::{content::Con, message::Message, update, Opt};
use iced::{button, executor, Application, Command};
use structopt::StructOpt;

pub(crate) struct App {
    pub(crate) opt: Opt,
    pub(crate) content: Con,
}

impl Application for App {
    type Message = Message;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        let opt = Opt::from_args();
        let app = Self {
            opt,
            content: Con::Empty(button::State::new()),
        };
        (app, Command::none())
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
