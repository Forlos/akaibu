use akaibu::{error::AkaibuError, magic::Archive, scheme::Scheme};
use anyhow::Context;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use scroll::Pread;
use std::io::{Read, Write};
use std::{fs::File, path::PathBuf};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt()]
struct Opt {
    /// Files to process
    #[structopt(required = true, name = "ARCHIVES", parse(from_os_str))]
    files: Vec<PathBuf>,
    #[structopt(
        short = "o",
        long = "output",
        parse(from_os_str),
        default_value = "ext/"
    )]

    /// Directory to output extracted files
    output_dir: PathBuf,

    /// Convert image files to commonly used format(PNG)
    #[structopt(long)]
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
            let mut contents = vec![0; 8];
            File::open(&file)?.read_exact(&mut contents)?;
            contents.pread::<u32>(0)?;

            let archive_magic = Archive::parse(&contents);
            if let Archive::NotRecognized = archive_magic {
                return Err(AkaibuError::UnrecognizedFormat(
                    file.clone(),
                    contents,
                )
                .into());
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

            let a = scheme.extract(&file)?;
            let progress_bar = init_progressbar(
                &format!("Extracting: {:?}", file),
                a.get_files().len() as u64,
            );

            a.get_files()
                .par_iter()
                .progress_with(progress_bar)
                .try_for_each(|f| {
                    let buf = a.extract(f.file_name)?;
                    let mut output_file_name = PathBuf::from(&opt.output_dir);
                    output_file_name.push(&f.file_name);
                    std::fs::create_dir_all(
                        &output_file_name
                            .parent()
                            .context("Could not get parent directory")?,
                    )?;
                    log::debug!(
                        "Extracting resource: {:?} {:X?}",
                        output_file_name,
                        f
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
