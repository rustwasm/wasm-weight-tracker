use failure::{Error, ResultExt};
use std::process::{Command, Stdio};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Benchmark {
    pub name: String,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
}

impl Benchmark {
    pub fn new(name: &str) -> Benchmark {
        Benchmark {
            name: name.to_string(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Input {
    CargoLock { contents: String },
    Git { url: String, rev: String },
    Rustc { rev: String },
    PackageJsonLock { contents: String },
    WasmPack { version: String },
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Output {
    pub bytes: u64,
    pub name: String,
}

pub fn run(cmd: &mut Command) -> Result<(), Error> {
    run_output(cmd.stdout(Stdio::inherit()))?;
    Ok(())
}

pub fn run_output(cmd: &mut Command) -> Result<String, Error> {
    log::debug!("running {:?}", cmd);
    let output = cmd
        .stderr(Stdio::inherit())
        .output()
        .context(format!("failed to run {:?}", cmd))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        failure::bail!("failed to execute {:?}\nstatus: {}", cmd, output.status)
    }
}
