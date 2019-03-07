# Rust and WebAssembly Weight Tracker

The purpose of this repository is to track the size of Rust and WebAssembly
generated files over time. This is a pretty broad peroggative, but the goal is
currently to:

* Watch "interesting" metrics of file size when graphed over time
* Help detect both regressions and improvements, allowing us to diagnose what
  happened
* In the case of regressions, quickly see the impact, evaluate the cause, and
  take appropriate action.

## Running a benchmark locally

You can run a benchmark locally with:

```
$ cargo run --bin collector -- measure out.json $bench1 $bench2 ...
```

This will dump relevant data into `out.json` for all of the benchmarks that are
executed.

## Building the site locally

You can build the website locally with:

```
$ cargo run --bin site -- --git tmpdir html/data.json
```

and afterwards you can host the file in the `html` folder with your favorite
static file serving utility (like `python -m SimpleHTTPServer` or `http`) and
browse the website.

# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
