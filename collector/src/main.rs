use failure::{bail, Error, ResultExt};
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};
use tempfile::TempDir;

#[derive(serde::Serialize)]
struct Benchmark {
    name: String,
    inputs: Vec<Input>,
    outputs: Vec<Output>,
}

impl Benchmark {
    fn new(name: &str) -> Benchmark {
        Benchmark {
            name: name.to_string(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum Input {
    CargoLock { contents: String },
    Git { url: String, rev: String },
    Rustc { rev: String },
    PackageJsonLock { contents: String },
    WasmPack { version: String },
}

#[derive(serde::Serialize)]
struct Output {
    bytes: u64,
    name: String,
}

struct Context<'a> {
    tmp: &'a Path,
    benchmarks: Vec<Benchmark>,
}

fn main() {
    env_logger::init();
    let dst = env::args_os().nth(1).unwrap();
    let temp = TempDir::new().unwrap();
    let root = if env::var("CI").is_ok() {
        temp.path().to_path_buf()
    } else {
        let p = env::current_dir().unwrap().join("build");
        fs::create_dir_all(&p).unwrap();
        p
    };
    let mut cx = Context {
        tmp: &root,
        benchmarks: Vec::new(),
    };
    let err = match cx.main(dst.as_ref()) {
        Ok(()) => return,
        Err(e) => e,
    };
    eprintln!("error: {}", err);
    for cause in err.iter_causes() {
        eprintln!("\tcaused by: {}", cause);
    }
    process::exit(1);
}

impl Context<'_> {
    fn main(&mut self, dst: &Path) -> Result<(), Error> {
        fs::create_dir_all(&self.cargo_target_dir())?;
        self.twiggy().context("failed to benchmark twiggy")?;
        self.game_of_life()
            .context("failed to benchmark game-of-life")?;
        self.rust_webpack_template()
            .context("failed to benchmark rust-webpack-template")?;

        fs::write(dst, serde_json::to_string(&self.benchmarks)?)?;
        Ok(())
    }

    fn twiggy(&mut self) -> Result<(), Error> {
        let mut b = Benchmark::new("twiggy");
        let root = self.git_clone("https://github.com/rustwasm/twiggy", &mut b)?;
        self.wasm_pack_build("twiggy_wasm_api", &root.join("wasm-api"), &mut b)?;
        self.benchmarks.push(b);
        Ok(())
    }

    fn game_of_life(&mut self) -> Result<(), Error> {
        let mut b = Benchmark::new("game-of-life");
        let root = self.git_clone("https://github.com/rustwasm/wasm_game_of_life", &mut b)?;
        self.wasm_pack_build("wasm_game_of_life", &root, &mut b)?;
        self.benchmarks.push(b);
        Ok(())
    }

    fn rust_webpack_template(&mut self) -> Result<(), Error> {
        let mut b = Benchmark::new("rust-webpack-template");
        let root = self.git_clone("https://github.com/rustwasm/rust-webpack-template", &mut b)?;
        self.wasm_pack_build("rust_webpack", &root.join("crate"), &mut b)?;
        self.npm_install(&root, &mut b)?;
        self.webpack_build(&root, &mut b)?;
        self.benchmarks.push(b);
        Ok(())
    }

    fn git_clone(&mut self, url: &str, b: &mut Benchmark) -> Result<PathBuf, Error> {
        log::debug!("git clone {}", url);
        let dst = self.tmp.join(&b.name);

        if !dst.exists() {
            self.run(Command::new("git").arg("clone").arg(url).arg(&dst))
                .context(format!("failed to clone {}", url))?;
        }
        let rev = self.run_output(
            Command::new("git")
                .arg("rev-parse")
                .arg("HEAD")
                .current_dir(&dst),
        )?;
        let rev = rev.trim().to_string();
        b.inputs.push(Input::Git {
            url: url.to_string(),
            rev,
        });

        Ok(dst)
    }

    fn wasm_pack_build(
        &mut self,
        crate_name: &str,
        root: &Path,
        b: &mut Benchmark,
    ) -> Result<(), Error> {
        log::debug!("wasm-pack build {:?}", root);
        let version = self.run_output(Command::new("wasm-pack").arg("--version"))?;
        b.inputs.push(Input::WasmPack { version });

        let rustc_version = self.run_output(Command::new("rustc").arg("-vV"))?;
        let rustc_commit = rustc_version
            .lines()
            .find(|l| l.starts_with("commit-hash: "))
            .expect("failed to find rustc commit hash")
            .split_whitespace()
            .nth(1)
            .expect("failed to find rustc commit hash");
        b.inputs.push(Input::Rustc {
            rev: rustc_commit.to_string(),
        });

        self.run(
            Command::new("wasm-pack")
                .arg("build")
                .current_dir(root)
                .env("CARGO_TARGET_DIR", self.cargo_target_dir()),
        )?;
        let contents = self.find_lockfile(root)?;
        b.inputs.push(Input::CargoLock { contents });

        let pkg = root.join("pkg");
        let js = pkg.join(crate_name).with_extension("js");
        let wasm = pkg.join(format!("{}_bg.wasm", crate_name));
        b.outputs.push(Output {
            bytes: js.metadata()?.len(),
            name: "wasm-bindgen js shim".to_string(),
        });
        b.outputs.push(Output {
            bytes: wasm.metadata()?.len(),
            name: "wasm-bindgen wasm".to_string(),
        });
        b.outputs.push(Output {
            bytes: self.gzip_size(&js)?,
            name: "wasm-bindgen js shim (gz)".to_string(),
        });
        b.outputs.push(Output {
            bytes: self.gzip_size(&wasm)?,
            name: "wasm-bindgen wasm (gz)".to_string(),
        });
        Ok(())
    }

    fn npm_install(&self, root: &Path, b: &mut Benchmark) -> Result<(), Error> {
        if !root.join("node_modules").exists() {
            self.run(Command::new("npm").arg("install").current_dir(&root))?;
        }
        let contents = fs::read_to_string(root.join("package-lock.json"))?;
        b.inputs.push(Input::PackageJsonLock { contents });
        Ok(())
    }

    fn webpack_build(&self, root: &Path, b: &mut Benchmark) -> Result<(), Error> {
        self.run(
            Command::new("npm")
                .arg("run")
                .arg("build")
                .arg("--")
                .arg("-p")
                .arg("--out-dir")
                .arg(root.join("dist"))
                .env("CARGO_TARGET_DIR", self.cargo_target_dir())
                .current_dir(root),
        );

        let mut js = 0;
        let mut js_gz = 0;
        let mut wasm = 0;
        let mut wasm_gz = 0;
        for file in root.join("dist").read_dir()? {
            let file = file?;
            let path = file.path();
            let size = file.metadata()?.len();
            match path.extension().and_then(|s| s.to_str()) {
                Some("js") => {
                    js += size;
                    js_gz += self.gzip_size(&path)?;
                }
                Some("wasm") => {
                    wasm += size;
                    wasm_gz += self.gzip_size(&path)?;
                }
                _ => {}
            }
        }
        b.outputs.push(Output {
            bytes: js,
            name: "webpack-generated js".to_string(),
        });
        b.outputs.push(Output {
            bytes: wasm,
            name: "webpack-generated wasm".to_string(),
        });
        b.outputs.push(Output {
            bytes: js_gz,
            name: "webpack-generated js (gz)".to_string(),
        });
        b.outputs.push(Output {
            bytes: wasm_gz,
            name: "webpack-generated wasm (gz)".to_string(),
        });
        Ok(())
    }

    fn cargo_target_dir(&self) -> PathBuf {
        self.tmp.join("target")
    }

    fn find_lockfile(&self, root: &Path) -> Result<String, Error> {
        let mut cur = root;
        loop {
            if let Ok(result) = fs::read_to_string(cur.join("Cargo.lock")) {
                return Ok(result);
            }
            match cur.parent() {
                Some(p) => cur = p,
                None => bail!("could not find `Cargo.lcok` in {:?}", root),
            }
        }
    }

    fn run(&self, cmd: &mut Command) -> Result<(), Error> {
        self.run_output(cmd.stdout(Stdio::inherit()))?;
        Ok(())
    }

    fn run_output(&self, cmd: &mut Command) -> Result<String, Error> {
        log::debug!("running {:?}", cmd);
        let output = cmd
            .stderr(Stdio::inherit())
            .output()
            .context(format!("failed to run {:?}", cmd))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            bail!("failed to execute {:?}\nstatus: {}", cmd, output.status)
        }
    }

    fn gzip_size(&self, path: &Path) -> Result<u64, Error> {
        let input = fs::read(path)?;
        let mut dst = Vec::new();
        let mut w = flate2::write::GzEncoder::new(&mut dst, flate2::Compression::default());
        w.write_all(&input)?;
        w.finish()?;
        Ok(dst.len() as u64)
    }
}
