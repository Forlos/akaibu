use crate::{
    app::App,
    logic::convert,
    logic::extract,
    logic::preview,
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
        Message::ConvertFile(file_entry) => {
            if let Content::ArchiveView(ref mut content) = app.content {
                let path = convert::convert_resource(
                    &content.archive,
                    &file_entry,
                    &app.opt.file,
                )
                .map_err(|_| {
                    akaibu::error::AkaibuError::Custom(format!(
                        "Convert not available for: {}",
                        file_entry.file_name
                    ))
                })?;
                return Ok(Command::perform(
                    futures::future::ready(Status::Success(format!(
                        "Converted: {:?}",
                        path
                    ))),
                    Message::SetStatus,
                ));
            };
        }
        Message::ExtractFile(file_entry) => {
            if let Content::ArchiveView(ref mut content) = app.content {
                let path = extract::extract_single_file(
                    &content.archive,
                    &file_entry,
                    &app.opt.file,
                )?;
                return Ok(Command::perform(
                    futures::future::ready(Status::Success(format!(
                        "Extracted: {:?}",
                        path
                    ))),
                    Message::SetStatus,
                ));
            };
        }
        Message::PreviewFile(file_entry) => {
            if let Content::ArchiveView(ref mut content) = app.content {
                let resource =
                    preview::get_resource_type(&content.archive, &file_entry)?;
                content.preview.set_resource(resource, file_entry.file_name);
                content.preview.set_visible(true);
            }
        }
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
        Message::ClosePreview => {
            if let Content::ArchiveView(ref mut content) = app.content {
                content.preview.set_visible(false);
            }
        }
        Message::Error(err) => {
            if let Content::ArchiveView(ref mut content) = app.content {
                content.set_status(Status::Error(err));
            }
        }
    };
    Ok(Command::none())
}
