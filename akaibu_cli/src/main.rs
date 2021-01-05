#![deny(
    rust_2018_idioms,
    unreachable_pub,
    unsafe_code,
    unused_imports,
    unused_mut,
    missing_debug_implementations
)]

use akaibu::{
    archive::FileEntry,
    magic::Archive,
    resource::{ResourceMagic, ResourceType},
    scheme::Scheme,
};
use anyhow::Context;
use colored::*;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::io::{Read, Write};
use std::{fs::File, path::PathBuf};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt()]
struct Opt {
    /// Files to process
    #[structopt(required = true, name = "ARCHIVES", parse(from_os_str))]
    files: Vec<PathBuf>,

    /// Directory to output extracted files
    #[structopt(
        short = "o",
        long = "output",
        parse(from_os_str),
        default_value = "ext/"
    )]
    output_dir: PathBuf,

    /// Convert resource files to commonly used formats
    #[structopt(short, long)]
    convert: bool,
}

fn main() {
    env_logger::init();
    let opt = Opt::from_args();

    match if opt.convert {
        convert_resource(&opt)
    } else {
        extract_archive(&opt)
    } {
        Ok(_) => (),
        Err(err) => log::error!("Error while extracting: {}", err),
    }
}

fn convert_resource(opt: &Opt) -> anyhow::Result<()> {
    let progress_bar =
        init_progressbar("Converting...", opt.files.len() as u64);

    opt.files
        .par_iter()
        .progress_with(progress_bar)
        .filter(|file| file.is_file())
        .try_for_each(|file| {
            let mut magic = vec![0; 16];
            File::open(&file)?.read_exact(&mut magic)?;

            let resource_magic = ResourceMagic::parse_magic(&magic);
            let mut contents = Vec::with_capacity(1 << 20);
            log::debug!("Converting: {:?}", file);
            File::open(&file)?.read_to_end(&mut contents)?;
            match resource_magic.parse(contents) {
                Ok(r) => write_resource(r, file),
                Err(err) => {
                    log::error!("{:?}: {}", file, err);
                    Ok(())
                }
            }
        })
}

fn extract_archive(opt: &Opt) -> anyhow::Result<()> {
    opt.files
        .iter()
        .filter(|file| file.is_file())
        .try_for_each(|file| {
            let mut magic = vec![0; 32];
            File::open(&file)?.read_exact(&mut magic)?;

            let archive_magic = Archive::parse(&magic);
            log::debug!("Archive: {:?}", archive_magic);
            let schemes = if let Archive::NotRecognized = archive_magic {
                println!(
                    "{}",
                    "Archive type could not be guessed. Please enter scheme manually:"
                        .yellow()
                );
                Archive::get_all_schemes()
            } else {
                archive_magic.get_schemes()
            };

            let scheme = if archive_magic.is_universal() {
                schemes.get(0).context("Scheme list is empty")?
            } else {
                schemes
                    .get(prompt_for_game(&schemes, &file))
                    .context("Could no get scheme from scheme list")?
            };
            log::debug!("Scheme {:?}", scheme);

            let (archive, dir) = match scheme.extract(&file) {
                Ok(archive) => archive,
                Err(err) => {
                    log::error!("{:?}: {}", file, err);
                    return Ok(());
                }
            };
            let files = dir
                .get_root_dir()
                .get_all_files()
                .cloned()
                .collect::<Vec<FileEntry>>();
            let progress_bar = init_progressbar(
                &format!("Extracting: {:?}", file),
                files.len() as u64,
            );

            files
                .par_iter()
                .progress_with(progress_bar)
                .try_for_each(|entry| {
                    let buf = archive.extract(entry)?;
                    let mut output_file_name = PathBuf::from(&opt.output_dir);
                    output_file_name.push(&entry.full_path);
                    std::fs::create_dir_all(
                        &output_file_name
                            .parent()
                            .context("Could not get parent directory")?,
                    )?;
                    log::debug!(
                        "Extracting resource: {:?} {:X?}",
                        output_file_name,
                        entry
                    );
                    File::create(output_file_name)?.write_all(&buf)?;
                    Ok(())
                })
        })
}

fn prompt_for_game(schemes: &[Box<dyn Scheme>], file_name: &PathBuf) -> usize {
    use read_input::prelude::*;

    let msg = schemes
        .iter()
        .enumerate()
        .map(|s| format!(" {}: {}\n", s.0, s.1.get_name()))
        .fold(
            format!("{:?}\nSelect game by typing number:\n", file_name),
            |mut v, s| {
                v += &s;
                v
            },
        );
    input::<usize>()
        .repeat_msg(msg)
        .err("Invalid input value".red())
        .inside_err(
            0..schemes.len(),
            format!("Please input value from 0 to {}", schemes.len() - 1).red(),
        )
        .get()
}

fn init_progressbar(prefix: &str, size: u64) -> ProgressBar {
    let progress_bar = ProgressBar::new(size).with_style(
        ProgressStyle::default_bar().template(
            " {spinner} {prefix} {wide_bar:} {pos:>6}/{len:6} ETA:[{eta}]",
        ),
    );
    progress_bar.set_prefix(prefix);
    progress_bar
}

fn write_resource(
    resource: ResourceType,
    file_name: &PathBuf,
) -> anyhow::Result<()> {
    match resource {
        ResourceType::RgbaImage { image } => {
            let mut new_file_name = file_name.clone();
            new_file_name.set_extension("png");
            image.save(new_file_name)?;
            Ok(())
        }
        ResourceType::Text(s) => {
            let mut new_file_name = file_name.clone();
            new_file_name.set_extension("txt");
            File::create(new_file_name)?.write_all(s.as_bytes())?;
            Ok(())
        }
        ResourceType::Other => Ok(()),
    }
}
