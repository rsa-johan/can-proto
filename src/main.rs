use std::{fs, path::{Path, PathBuf}, process::ExitCode};

use proto_c::{compile_file, compile_package, ErrorSignals, Generated};

const USAGE: &str = "\
Compile CAN proto (.cp) definitions into Rust modules and C headers.

Usage: proto_c [OPTIONS] <INPUT>...

Arguments:
  <INPUT>...           a directory is compiled as a package: <name>/mod.rs plus one
                       .rs/.h per file (x.cp -> x, x.inc.cp -> x_inc); a single
                       file is compiled self-contained with its #inc= files inlined

Options:
  -o, --out <DIR>      output directory [default: generated]
      --rust-out <DIR> directory for .rs files [default: <out>/rust]
      --c-out <DIR>    directory for .h files [default: <out>/c]
  -l, --lang <LANG>    rust | c | all [default: all]
      --check          only parse and validate, write nothing
  -h, --help           print this help";

struct Args {
    inputs: Vec<PathBuf>,
    rust_out: Option<PathBuf>,
    c_out: Option<PathBuf>,
    check: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut inputs = Vec::new();
    let (mut out, mut rust_out, mut c_out) = (PathBuf::from("generated"), None, None);
    let (mut rust, mut c, mut check) = (true, true, false);

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = |flag: &str| it.next().ok_or_else(|| format!("missing value for `{flag}`"));
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            "-o" | "--out" => out = value(&arg)?.into(),
            "--rust-out" => rust_out = Some(value(&arg)?.into()),
            "--c-out" => c_out = Some(value(&arg)?.into()),
            "-l" | "--lang" => match value(&arg)?.as_str() {
                "rust" => (rust, c) = (true, false),
                "c" => (rust, c) = (false, true),
                "all" => (rust, c) = (true, true),
                other => return Err(format!("unknown language `{other}`")),
            },
            "--check" => check = true,
            s if s.starts_with('-') => return Err(format!("unknown option `{s}`")),
            _ => inputs.push(PathBuf::from(arg)),
        }
    }
    if inputs.is_empty() {
        return Err("no input files".into());
    }
    Ok(Args {
        inputs,
        rust_out: rust.then(|| rust_out.unwrap_or_else(|| out.join("rust"))),
        c_out: c.then(|| c_out.unwrap_or_else(|| out.join("c"))),
        check,
    })
}

fn write(dir: &Path, name: &str, content: &str) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let path = dir.join(name);
    fs::write(&path, content).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("  wrote {}", path.display());
    Ok(())
}

fn emit(args: &Args, sub: &str, module: &str, g: &Generated) -> Result<(), String> {
    if args.check {
        return Ok(());
    }
    if let Some(dir) = &args.rust_out {
        write(&dir.join(sub), &format!("{module}.rs"), &g.rust)?;
    }
    if let Some(dir) = &args.c_out {
        write(&dir.join(sub), &format!("{module}.h"), &g.c)?;
    }
    Ok(())
}

fn run(args: Args) -> Result<(), String> {
    let mut failed = 0;
    for input in &args.inputs {
        let result = if input.is_dir() {
            compile_package(input).map(|pkg| {
                println!("compiled package `{}` ({} file(s))", pkg.name, pkg.units.len());
                for s in &pkg.skipped {
                    println!("  skipped {s} (ignored by header)");
                }
                let mut out = Ok(());
                if let (Some(dir), false) = (&args.rust_out, args.check) {
                    out = write(&dir.join(&pkg.name), "mod.rs", &pkg.mod_rs);
                }
                for (module, g) in &pkg.units {
                    out = out.and_then(|_| emit(&args, &pkg.name, module, g));
                }
                if let Err(e) = out {
                    eprintln!("error: {e}");
                    failed += 1;
                }
            })
        } else {
            compile_file(input).map(|g| {
                println!("compiled {}", input.display());
                let name = input.file_name().and_then(|s| s.to_str()).unwrap_or("proto.cp");
                let module = match name.strip_suffix(".inc.cp") {
                    Some(base) => format!("{base}_inc"),
                    None => name.strip_suffix(".cp").unwrap_or(name).to_string(),
                };
                if let Err(e) = emit(&args, "", &module, &g) {
                    eprintln!("error: {e}");
                    failed += 1;
                }
            })
        };
        match result {
            Ok(()) => {}
            Err(ErrorSignals::NoError) => println!("skipped {} (ignored by header)", input.display()),
            Err(e) => {
                eprintln!("error: {e}");
                failed += 1;
            }
        }
    }

    if failed > 0 {
        return Err(format!("{failed} input(s) failed"));
    }
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}\n\n{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
