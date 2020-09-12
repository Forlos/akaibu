use akaibu::{
    error::AkaibuError,
    magic::Archive,
    resource::{ResourceMagic, ResourceType},
    scheme::Scheme,
};
use anyhow::Context;
use image::ImageBuffer;
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

    match run(&opt) {
        Ok(_) => (),
        Err(err) => log::error!("Error while extracting: {}", err),
    }
}

fn run(opt: &Opt) -> anyhow::Result<()> {
    opt.files
        .iter()
        .filter(|file| file.is_file())
        .try_for_each(|file| {
            let mut magic = vec![0; 32];
            File::open(&file)?.read_exact(&mut magic)?;

            let archive_magic = Archive::parse(&magic);
            if let Archive::NotRecognized = archive_magic {
                if opt.convert {
                    let resource_magic = ResourceMagic::parse_magic(&magic);
                    let mut contents = Vec::with_capacity(1 << 20);
                    File::open(&file)?.read_to_end(&mut contents)?;
                    match resource_magic.parse(contents) {
                        Ok(r) => return write_resource(r, file),
                        Err(err) => {
                            log::error!("{:?}: {}", file, err);
                            return Ok(());
                        }
                    }
                } else {
                    let err =
                        AkaibuError::UnrecognizedFormat(file.clone(), magic);
                    log::error!("{}", err);
                    return Ok(());
                }
            }

            log::debug!("Archive: {:?}", archive_magic);
            let schemes = archive_magic.get_schemes();
            let scheme = if archive_magic.is_universal() {
                schemes.get(0).context("Scheme list is empty")?
            } else {
                schemes
                    .get(prompt_for_game(&schemes, &file))
                    .context("Could no get scheme from scheme list")?
            };
            log::debug!("Scheme {:?}", scheme);

            let archive = match scheme.extract(&file) {
                Ok(archive) => archive,
                Err(err) => {
                    log::error!("{:?}: {}", file, err);
                    return Ok(());
                }
            };
            let progress_bar = init_progressbar(
                &format!("Extracting: {:?}", file),
                archive.get_files().len() as u64,
            );

            archive
                .get_files()
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
    use colored::*;
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
