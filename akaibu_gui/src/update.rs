use crate::{
    app::App,
    archive::ArchiveContent,
    content::Content,
    message::{Message, Scene},
};
use iced::Command;

pub(crate) fn handle_message(
    app: &mut App,
    message: Message,
) -> anyhow::Result<Command<Message>> {
    log::info!("{:?}", message);
    match message {
        Message::OpenDirectory(dir_name) => match app.content {
            Content::ArchiveView(ref mut content) => {
                content.move_dir(dir_name);
            }
            _ => {}
        },
        Message::BackDirectory => match app.content {
            Content::ArchiveView(ref mut content) => {
                content.back_dir();
            }
            _ => {}
        },
        Message::ExtractFile(file) => {}
        Message::PreviewFile(file) => {}
        Message::ConvertFile(file) => {}
        Message::Empty => (),
        Message::Error(_) => (),
        Message::ExtractAll => {
            // TODO show progress to user and move logic to different file
            // app.archive
            //     .get_files()
            //     .iter()
            //     .enumerate()
            //     .for_each(|(i, e)| {
            //         let buf = app.archive.extract(e).unwrap();
            //         let mut output_file_name = PathBuf::from("ext/");
            //         output_file_name.push(&e.file_name);
            //         std::fs::create_dir_all(
            //             &output_file_name.parent().unwrap(),
            //         )
            //         .unwrap();
            //         File::create(output_file_name)
            //             .unwrap()
            //             .write_all(&buf)
            //             .unwrap();
            //     });
        }
        Message::UpdateScrollbar(i) => {
            // app.content.extract_all_progress = i;
        }
        Message::MoveScene(scene) => match scene {
            Scene::ArchiveView(scheme) => {
                app.content = Content::ArchiveView(ArchiveContent::new(
                    scheme.extract(&app.opt.file)?,
                ));
            }
        },
    };
    Ok(Command::none())
}
