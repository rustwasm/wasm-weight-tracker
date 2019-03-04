use failure::{bail, Error};
use std::cmp;
use shared::*;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

const USAGE: &'static str = "
Generate website

Usage:
    site [options] <output>
    site -h | --help

Options:
    --git DIR                    Clone data into `DIR`
    --local DIR                  Use data in `DIR`
    -h --help                    Show this screen.
";

#[derive(Debug, serde::Deserialize)]
struct Args {
    arg_output: PathBuf,
    flag_git: Option<PathBuf>,
    flag_local: Option<PathBuf>,
}

#[derive(serde::Serialize)]
struct Build {
    date: String,
    data: Vec<Benchmark>,
}

fn main() {
    env_logger::init();
    let args: Args = docopt::Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());
    let err = match rmain(&args) {
        Ok(()) => return,
        Err(e) => e,
    };
    eprintln!("error: {}", err);
    for cause in err.iter_causes() {
        eprintln!("\tcaused by: {}", cause);
    }
    process::exit(1);
}

fn rmain(args: &Args) -> Result<(), Error> {
    let data_dir = if let Some(dir) = &args.flag_git {
        if !dir.exists() {
            run(Command::new("git")
                .arg("clone")
                .arg("https://github.com/rustwasm/wasm-weight-tracker-data")
                .arg(dir))?;
        }
        dir
    } else if let Some(dir) = &args.flag_local {
        dir
    } else {
        bail!("must specify --local or --git")
    };
    let builds = read_data(&data_dir.join("builds"))?;

    let len = builds.len();
    let builds = &builds[len - cmp::min(len, 60)..len];
    fs::write(&args.arg_output, serde_json::to_string(&builds)?)?;
    Ok(())
}

fn read_data(dir: &Path) -> Result<Vec<Build>, Error> {
    let mut builds = Vec::new();
    for entry in dir.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().unwrap().to_str().unwrap();
        if !name.ends_with(".json.gz") {
            continue;
        }
        log::debug!("parsing: {}", name);

        let file_date = name.split('.').next().unwrap();
        let mut parts = file_date.split('-');
        let date = iso8601::datetime(&format!(
            "{}-{}-{}T{}",
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap(),
            {
                let digits = parts.next().unwrap();
                format!("{}:{}", &digits[..2], &digits[2..])
            },
        )).unwrap();

        let contents = fs::read(&path)?;
        let mut json = String::new();
        flate2::read::GzDecoder::new(&contents[..]).read_to_string(&mut json)?;
        builds.push(Build {
            date: date.to_string(),
            data: serde_json::from_str(&json)?,
        });
    }

    log::debug!("found {} builds", builds.len());
    builds.sort_by_key(|b| b.date.clone());
    Ok(builds)
}
