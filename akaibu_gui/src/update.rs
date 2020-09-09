use crate::{
    app::App,
    logic::extract,
    message::Status,
    message::{Message, Scene},
    ui::archive::ArchiveContent,
    ui::content::Content,
};
use iced::{futures, Command};

pub(crate) fn handle_message(
    app: &mut App,
    message: Message,
) -> anyhow::Result<Command<Message>> {
    log::info!("{:?}", message);
    match message {
        Message::OpenDirectory(dir_name) => {
            if let Content::ArchiveView(ref mut content) = app.content {
                content.move_dir(dir_name);
            }
        }
        Message::BackDirectory => {
            if let Content::ArchiveView(ref mut content) = app.content {
                content.back_dir();
            }
        }
        Message::ConvertFile(file_entry) => {}
        Message::ExtractFile(file_entry) => {
            if let Content::ArchiveView(ref mut content) = app.content {
                extract::extract_single_file(
                    &content.archive,
                    &file_entry,
                    &app.opt.file,
                )?
            };
            return Ok(Command::perform(
                futures::future::ready(Status::Success(format!(
                    "Extracted: {}",
                    file_entry.file_name
                ))),
                Message::SetStatus,
            ));
        }
        Message::PreviewFile(file_entry) => {}
        Message::ExtractAll => {
            // TODO make extracting async
            if let Content::ArchiveView(ref mut content) = app.content {
                return Ok(Command::perform(
                    futures::future::ready(extract::extract_all(
                        &content.archive,
                        &app.opt.file,
                    )?),
                    |_| {
                        Message::SetStatus(Status::Success(String::from(
                            "Extracted all!",
                        )))
                    },
                ));
            };
        }
        Message::UpdateScrollbar(progress) => {
            if let Content::ArchiveView(ref mut content) = app.content {
                content.set_progress(progress);
            }
        }
        Message::MoveScene(scene) => match scene {
            Scene::ArchiveView(scheme) => {
                app.content = Content::ArchiveView(ArchiveContent::new(
                    scheme.extract(&app.opt.file)?,
                ));
            }
        },
        Message::SetStatus(status) => {
            if let Content::ArchiveView(ref mut content) = app.content {
                content.set_status(status);
            }
        }
        Message::Empty => (),
        Message::Error(err) => {
            if let Content::ArchiveView(ref mut content) = app.content {
                content.set_status(Status::Error(err));
            }
        }
    };
    Ok(Command::none())
}
