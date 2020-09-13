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
use iced::Command;

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
                return Ok(Command::perform(
                    convert::convert_resource(
                        content.archive.clone(),
                        file_entry,
                        app.opt.file.clone(),
                    ),
                    |result| match result {
                        Ok(path) => Message::SetStatus(Status::Success(
                            format!("Extracted: {:?}", path),
                        )),
                        Err(err) => Message::SetStatus(Status::Error(format!(
                            "{}",
                            err
                        ))),
                    },
                ));
            };
        }
        Message::ExtractFile(file_entry) => {
            if let Content::ArchiveView(ref mut content) = app.content {
                return Ok(Command::perform(
                    extract::extract_single_file(
                        content.archive.clone(),
                        file_entry,
                        app.opt.file.clone(),
                    ),
                    |result| match result {
                        Ok(path) => Message::SetStatus(Status::Success(
                            format!("Extracted: {:?}", path),
                        )),
                        Err(err) => Message::SetStatus(Status::Error(format!(
                            "Error while extracting: {}",
                            err
                        ))),
                    },
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
            if let Content::ArchiveView(ref mut content) = app.content {
                let commands = vec![
                    Command::perform(async {}, |_| {
                        Message::SetStatus(Status::Normal(
                            "Extracting...".to_string(),
                        ))
                    }),
                    Command::perform(
                        extract::extract_all(
                            content.archive.clone(),
                            content
                                .navigable_dir
                                .get_root_dir()
                                .get_all_files()
                                .cloned()
                                .collect(),
                            app.opt.file.clone(),
                        ),
                        |result| match result {
                            Ok(_) => Message::SetStatus(Status::Success(
                                String::from("Extracted all!"),
                            )),
                            Err(err) => Message::SetStatus(Status::Error(
                                format!("Error while extracting: {}", err),
                            )),
                        },
                    ),
                ];
                return Ok(Command::batch(commands));
            };
        }
        Message::UpdateScrollbar(progress) => {
            if let Content::ArchiveView(ref mut content) = app.content {
                content.set_progress(progress);
            }
        }
        Message::MoveScene(scene) => match scene {
            Scene::ArchiveView(scheme) => {
                let (archive, dir) = scheme.extract(&app.opt.file)?;
                app.content =
                    Content::ArchiveView(ArchiveContent::new(archive, dir));
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
        Message::PatternChanged(pattern) => {
            if let Content::ArchiveView(ref mut content) = app.content {
                content.pattern = pattern;
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
