use crate::{
    app::App,
    logic::convert,
    logic::extract,
    logic::preview,
    message::Status,
    message::{Message, Scene},
    ui::archive::ArchiveContent,
    ui::{content::Content, resource::ResourceContent},
};
use extract::extract_all;
use iced::Command;

pub(crate) fn handle_message(
    app: &mut App,
    message: Message,
) -> anyhow::Result<Command<Message>> {
    log::info!("{:?}", message);
    match message {
        Message::OpenDirectory(dir_name) => {
            if let Content::ArchiveView(ref mut content) = app.content {
                content.move_dir(dir_name)?;
            }
        }
        Message::BackDirectory => {
            if let Content::ArchiveView(ref mut content) = app.content {
                content.back_dir()?;
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
                            format!("Converted: {:?}", path),
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
                return Ok(Command::perform(
                    preview::get_resource_type(
                        content.archive.clone(),
                        file_entry.clone(),
                    ),
                    move |result| match result {
                        Ok(resource) => Message::OpenPreview(
                            resource,
                            file_entry.file_name.clone(),
                        ),
                        Err(err) => Message::SetStatus(Status::Error(format!(
                            "{}",
                            err
                        ))),
                    },
                ));
            }
        }
        Message::ExtractAll => {
            if let Content::ArchiveView(ref mut content) = app.content {
                let mut commands = vec![Command::perform(async {}, |_| {
                    Message::SetStatus(Status::Normal(
                        "Extracting...".to_string(),
                    ))
                })];
                if content.convert_all {
                    commands.push(Command::perform(
                        extract::extract_all_with_convert(
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
                            Ok(path) => Message::SetStatus(Status::Success(
                                format!("Extracted all! {:?}", path),
                            )),
                            Err(err) => Message::SetStatus(Status::Error(
                                format!("Error while extracting: {}", err),
                            )),
                        },
                    ));
                } else {
                    commands.push(Command::perform(
                        extract_all(
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
                            Ok(path) => Message::SetStatus(Status::Success(
                                format!("Extracted all! {:?}", path),
                            )),
                            Err(err) => Message::SetStatus(Status::Error(
                                format!("Error while extracting: {}", err),
                            )),
                        },
                    ));
                }
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
                app.content = Content::ArchiveView(Box::new(
                    ArchiveContent::new(archive, dir),
                ));
            }
            Scene::ResourceView(scheme, file_path) => {
                let resource = scheme.convert(&app.opt.file)?;
                app.content = Content::ResourceView(ResourceContent::new(
                    resource, file_path,
                ));
            }
        },
        Message::SetStatus(status) => match app.content {
            Content::ArchiveView(ref mut content) => {
                content.set_status(status);
            }
            Content::SchemeView(ref mut content) => {
                content.set_status(status);
            }
            Content::ResourceView(ref mut content) => {
                content.set_status(status);
            }
            Content::ResourceSchemeView(ref mut content) => {
                content.set_status(status);
            }
        },
        Message::OpenPreview(resource, file_name) => {
            if let Content::ArchiveView(ref mut content) = app.content {
                content.preview.set_resource(resource, file_name);
                content.preview.set_visible(true);
            }
        }
        Message::ClosePreview => {
            if let Content::ArchiveView(ref mut content) = app.content {
                content.preview.set_visible(false);
            }
        }
        Message::ConvertAllToggle(convert_all) => {
            if let Content::ArchiveView(ref mut content) = app.content {
                content.convert_all = convert_all;
            }
        }
        Message::PatternChanged(pattern) => {
            if let Content::ArchiveView(ref mut content) = app.content {
                content.pattern = pattern;
            }
        }
        Message::FormatChanged(format) => {
            if let Content::ResourceView(ref mut content) = app.content {
                content.format = format;
            }
        }
        Message::SaveResource => {
            if let Content::ResourceView(ref mut content) = app.content {
                return Ok(Command::perform(
                    iced::futures::future::ready(
                        convert::write_resource_with_format(
                            content.resource.clone(),
                            content.file_name.clone(),
                            content.format,
                        ),
                    ),
                    |result| match result {
                        Ok(path) => Message::SetStatus(Status::Success(
                            format!("Saved: {:?}", path),
                        )),
                        Err(err) => Message::SetStatus(Status::Error(format!(
                            "{}",
                            err
                        ))),
                    },
                ));
            }
        }
        Message::Error(err) => match app.content {
            Content::ArchiveView(ref mut content) => {
                content.set_status(Status::Error(err));
            }
            Content::SchemeView(ref mut content) => {
                content.set_status(Status::Error(err));
            }
            Content::ResourceView(ref mut content) => {
                content.set_status(Status::Error(err));
            }
            Content::ResourceSchemeView(ref mut content) => {
                content.set_status(Status::Error(err));
            }
        },
    };
    Ok(Command::none())
}
