use crate::{app::App, message::Message};
use iced::Command;
use std::{fs::File, io::Write, path::PathBuf};

pub(crate) fn handle_message(
    app: &mut App,
    message: Message,
) -> anyhow::Result<Command<Message>> {
    match message {
        Message::ExtractFile(file_name) => (),
        Message::PreviewFile(file_name) => {
            // let contents = app.archive.extract(&file_name);
        }
        Message::Empty => (),
        Message::Error(_) => (),
        Message::ExtractAll => {
            // TODO show progress to user and move logic to different file
            app.archive
                .get_files()
                .iter()
                .enumerate()
                .for_each(|(i, e)| {
                    let buf = app.archive.extract(e).unwrap();
                    let mut output_file_name = PathBuf::from("ext/");
                    output_file_name.push(&e.file_name);
                    std::fs::create_dir_all(
                        &output_file_name.parent().unwrap(),
                    )
                    .unwrap();
                    File::create(output_file_name)
                        .unwrap()
                        .write_all(&buf)
                        .unwrap();
                });
        }
        Message::UpdateScrollbar(i) => {
            println!("{}", i);
            app.content.extract_all_progress = i;
        }
    };
    Ok(Command::none())
}
