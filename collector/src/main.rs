use failure::{bail, Error, ResultExt};
use shared::*;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use tempfile::TempDir;

struct Context<'a> {
    tmp: &'a Path,
    benchmarks: Vec<Benchmark>,
}

const USAGE: &'static str = "
Collect benchmark data about size of various Rust/wasm projects

Usage:
    collector measure [options] <output> <benchmarks>...
    collector merge [options] <output> <inputs>...
    collector -h | --help

Options:
    -h --help                    Show this screen.
    --tmp-dir DIR                Temporary build directory
";

#[derive(Debug, serde::Deserialize)]
struct Args {
    cmd_measure: bool,
    cmd_merge: bool,
    arg_output: PathBuf,
    flag_tmp_dir: Option<PathBuf>,
    arg_benchmarks: Vec<String>,
    arg_inputs: Vec<String>,
}

fn main() {
    env_logger::init();
    let args: Args = docopt::Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    let temp = TempDir::new().unwrap();
    let root = match &args.flag_tmp_dir {
        Some(p) => p.as_path(),
        None => temp.path(),
    };
    fs::create_dir_all(root).unwrap();
    let mut cx = Context {
        tmp: &root,
        benchmarks: Vec::new(),
    };
    let result = if args.cmd_measure {
        cx.measure(&args.arg_output, &args.arg_benchmarks)
    } else {
        cx.merge(&args.arg_output, &args.arg_inputs)
    };
    let err = match result {
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
    fn measure(&mut self, dst: &Path, benchmarks: &[String]) -> Result<(), Error> {
        fs::create_dir_all(&self.cargo_target_dir())?;

        for benchmark in benchmarks {
            match benchmark.as_str() {
                "twiggy" => self.twiggy()?,
                "dodrio_todomvc" => self.dodrio_todomvc()?,
                "source_map_mappings" => self.source_map_mappings()?,
                "game_of_life" => self.game_of_life()?,
                "rust_webpack_template" => self.rust_webpack_template()?,
                "squoosh_rotate" => self.squoosh_rotate()?,
                s => bail!("unknown benchmark: {}", s),
            }
        }

        fs::write(dst, serde_json::to_string(&self.benchmarks)?)?;
        Ok(())
    }

    fn merge(&mut self, dst: &Path, inputs: &[String]) -> Result<(), Error> {
        let mut benchmarks = Vec::new();

        for input in inputs {
            let json = fs::read_to_string(input).context(format!("failed to read {}", input))?;
            let json: Vec<Benchmark> = serde_json::from_str(&json)?;
            benchmarks.extend(json);
        }

        fs::write(dst, serde_json::to_string(&benchmarks)?)?;
        Ok(())
    }

    fn twiggy(&mut self) -> Result<(), Error> {
        let mut b = Benchmark::new("twiggy");
        let root = self.git_clone("https://github.com/rustwasm/twiggy", &mut b)?;
        self.wasm_pack_build("twiggy_wasm_api", &root.join("wasm-api"), &mut b)?;
        self.benchmarks.push(b);
        Ok(())
    }

    fn dodrio_todomvc(&mut self) -> Result<(), Error> {
        let mut b = Benchmark::new("dodrio-todomvc");
        let root = self.git_clone("https://github.com/fitzgen/dodrio", &mut b)?;
        self.wasm_pack_build("dodrio_todomvc", &root.join("examples/todomvc"), &mut b)?;
        self.benchmarks.push(b);
        Ok(())
    }

    fn source_map_mappings(&mut self) -> Result<(), Error> {
        let mut b = Benchmark::new("source-map-mappings");
        let root = self.git_clone("https://github.com/fitzgen/source-map-mappings", &mut b)?;
        self.cargo_build(
            &root.join("source-map-mappings-wasm-api"),
            "source_map_mappings_wasm_api",
            &mut b,
        )?;
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

    fn squoosh_rotate(&mut self) -> Result<(), Error> {
        let mut b = Benchmark::new("squoosh-rotate");
        let root = self.git_clone("https://github.com/GoogleChromeLabs/squoosh", &mut b)?;
        self.cargo_build(&root.join("codecs/rotate"), "rotate", &mut b)?;
        self.benchmarks.push(b);
        Ok(())
    }

    fn git_clone(&mut self, url: &str, b: &mut Benchmark) -> Result<PathBuf, Error> {
        log::debug!("git clone {}", url);
        let dst = self.tmp.join(&b.name);

        if !dst.exists() {
            run(Command::new("git").arg("clone").arg(url).arg(&dst))
                .context(format!("failed to clone {}", url))?;
        }
        let rev = run_output(
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
        let version = run_output(Command::new("wasm-pack").arg("--version"))?;
        b.inputs.push(Input::WasmPack { version });

        self.add_rustc_version(b)?;

        run(Command::new("wasm-pack")
            .arg("build")
            .current_dir(root)
            .env("CARGO_TARGET_DIR", self.cargo_target_dir()))?;
        self.add_lockfile(root, b)?;

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

    fn cargo_build(
        &mut self,
        manifest_dir: &Path,
        crate_name: &str,
        b: &mut Benchmark,
    ) -> Result<(), Error> {
        log::debug!("cargo build {:?}", manifest_dir);
        self.add_rustc_version(b)?;

        run(Command::new("cargo")
            .arg("build")
            .arg("--release")
            .arg("--target")
            .arg("wasm32-unknown-unknown")
            .current_dir(manifest_dir)
            .env("CARGO_TARGET_DIR", self.cargo_target_dir()))?;
        self.add_lockfile(manifest_dir, b)?;

        let wasm = self.cargo_target_dir()
            .join("wasm32-unknown-unknown")
            .join("release")
            .join(crate_name)
            .with_extension("wasm");

        // Strip out the debug and name custom sections as we don't want to
        // account for those sizes in this measurement.
        run(Command::new("wasm-strip").arg(&wasm))?;

        b.outputs.push(Output {
            bytes: wasm.metadata()?.len(),
            name: "wasm".to_string(),
        });
        b.outputs.push(Output {
            bytes: self.gzip_size(&wasm)?,
            name: "wasm (gz)".to_string(),
        });
        Ok(())
    }

    fn add_rustc_version(&self, b: &mut Benchmark) -> Result<(), Error> {
        let rustc_version = run_output(Command::new("rustc").arg("-vV"))?;
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
        Ok(())
    }

    fn add_lockfile(&self, dir: &Path, b: &mut Benchmark) -> Result<(), Error> {
        let contents = self.find_lockfile(dir)?;
        let toml: toml::Value = toml::from_str(&contents)?;
        let json = serde_json::to_string(&toml)?;
        b.inputs.push(Input::CargoLock { contents: json });
        Ok(())
    }

    fn npm_install(&self, root: &Path, b: &mut Benchmark) -> Result<(), Error> {
        if !root.join("node_modules").exists() {
            run(Command::new("npm").arg("install").current_dir(&root))?;
        }
        let contents = fs::read_to_string(root.join("package-lock.json"))?;
        b.inputs.push(Input::PackageJsonLock { contents });
        Ok(())
    }

    fn webpack_build(&self, root: &Path, b: &mut Benchmark) -> Result<(), Error> {
        run(Command::new("npm")
            .arg("run")
            .arg("build")
            .arg("--")
            .arg("-p")
            .arg("--out-dir")
            .arg(root.join("dist"))
            .env("CARGO_TARGET_DIR", self.cargo_target_dir())
            .current_dir(root))?;

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

    fn gzip_size(&self, path: &Path) -> Result<u64, Error> {
        let input = fs::read(path)?;
        let mut dst = Vec::new();
        let mut w = flate2::write::GzEncoder::new(&mut dst, flate2::Compression::default());
        w.write_all(&input)?;
        w.finish()?;
        Ok(dst.len() as u64)
    }
}
