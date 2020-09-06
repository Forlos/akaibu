use crate::{
    app::App,
    archive::ArchiveContent,
    content::Content,
    message::{Message, Scene},
};
use akaibu::magic;
use iced::Command;
use std::{fs::File, io::Read};

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
            println!("{}", i);
            // app.content.extract_all_progress = i;
        }
        Message::MoveScene(scene) => match scene {
            Scene::ArchiveView => {
                // let file = app.opt.file;

                let mut magic = vec![0; 32];
                File::open(&app.opt.file)
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
                let archive = scheme.extract(&app.opt.file).unwrap();
                app.content =
                    Content::ArchiveView(ArchiveContent::new(archive));
            }
        },
    };
    Ok(Command::none())
}
