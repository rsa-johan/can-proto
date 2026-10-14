mod protocols;
mod common;
mod tools;
mod error;
mod init;
mod codegen;

use std::path::{Path, PathBuf};

use codegen::{c, rust, Layout};
use tools::{
    ast::{Item, Proto},
    parser::parse,
    resolve::{check_overlaps, resolve, FilterIr, Model},
    syntax::{is_ident, is_reserved},
};

pub use crate::common::PROTO_EXT;
pub use crate::error::ErrorSignals;

type Result<T> = std::result::Result<T, ErrorSignals>;

const INC_EXT: &str = ".inc.cp";

/// Generated sources for one `.cp` / `.inc.cp` file.
#[derive(Debug, Clone)]
pub struct Generated {
    pub rust: String,
    pub c: String,
}

/// A compiled proto directory.
///
/// Rust: `<name>/mod.rs` + `<name>/<module>.rs`. C: `<name>/<module>.h`.
/// `x.cp` becomes module `x`, `x.inc.cp` becomes `x_inc`.
#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    /// Shared prelude and module declarations.
    pub mod_rs: String,
    /// (module name, output) for every file.
    pub units: Vec<(String, Generated)>,
    /// Files skipped by an `#ignore=` header.
    pub skipped: Vec<String>,
}

struct Dep {
    module: String,
    path: PathBuf,
    proto: Proto,
}

fn prefixed(name: &str, e: ErrorSignals) -> ErrorSignals {
    match e {
        ErrorSignals::NoError => e,
        e => ErrorSignals::Generic(format!("{name}: {e}")),
    }
}

fn file_name(path: &Path) -> &str {
    path.file_name().and_then(|s| s.to_str()).unwrap_or("proto.cp")
}

fn is_inc(path: &Path) -> bool {
    file_name(path).ends_with(INC_EXT)
}

fn module_name(name: String, what: &str) -> Result<String> {
    if !is_ident(&name) || is_reserved(&name) || name == "mod" {
        return Err(ErrorSignals::Generic(format!("{what} name `{name}` is not usable as a Rust module / C header name")));
    }
    Ok(name)
}

fn module_of(path: &Path) -> Result<String> {
    let name = file_name(path);
    let module = match name.strip_suffix(INC_EXT) {
        Some(base) => format!("{base}_inc"),
        None => name.strip_suffix(&format!(".{PROTO_EXT}")).unwrap_or(name).to_string(),
    };
    module_name(module, "file")
}

fn load(path: &Path) -> Result<Proto> {
    let label = path.display().to_string();
    let src = std::fs::read_to_string(path).map_err(|e| ErrorSignals::Generic(format!("{label}: {e}")))?;
    parse(&src).map_err(|e| prefixed(&label, e))
}

/// Transitive `#inc=` closure of `proto`, dependencies first.
fn closure(dir: &Path, proto: &Proto, label: &str) -> Result<Vec<Dep>> {
    fn visit(dir: &Path, name: &str, ln: usize, label: &str, stack: &mut Vec<String>, out: &mut Vec<Dep>) -> Result<()> {
        let module = format!("{name}_inc");
        if out.iter().any(|d| d.module == module) {
            return Ok(());
        }
        if stack.iter().any(|s| s == name) {
            let at = ErrorSignals::at(ln, format!("include cycle: {} -> {name}", stack.join(" -> ")));
            return Err(prefixed(label, at));
        }
        let path = dir.join(format!("{name}{INC_EXT}"));
        if !path.is_file() {
            return Err(prefixed(label, ErrorSignals::at(ln, format!("include `{name}` not found ({})", path.display()))));
        }
        let proto = match load(&path) {
            Err(ErrorSignals::NoError) => {
                return Err(prefixed(label, ErrorSignals::at(ln, format!("`{name}` is marked `#ignore=`"))));
            }
            r => r?,
        };
        stack.push(name.to_string());
        let inner = path.display().to_string();
        for (n, l) in &proto.includes {
            visit(dir, n, *l, &inner, stack, out)?;
        }
        stack.pop();
        out.push(Dep { module, path, proto });
        Ok(())
    }

    let mut out = Vec::new();
    for (n, l) in &proto.includes {
        visit(dir, n, *l, label, &mut Vec::new(), &mut out)?;
    }
    Ok(out)
}

/// Resolves a file with its includes (inlined unless `deps_external`) and checks filter overlaps.
fn resolve_unit(dir: &Path, path: &Path, proto: &Proto, deps_external: bool) -> Result<(Vec<Dep>, Model)> {
    let label = path.display().to_string();
    if is_inc(path) {
        if let Some(item) = proto.items.iter().find(|i| matches!(i, Item::Frame(_) | Item::Fn(_))) {
            let e = ErrorSignals::at(item.line(), "include files may only contain `type`, `def`, `spec` and `filter`");
            return Err(prefixed(&label, e));
        }
    }
    let deps = closure(dir, proto, &label)?;
    let mut scope: Vec<(&Proto, bool)> = deps.iter().map(|d| (&d.proto, deps_external)).collect();
    scope.push((proto, false));
    let model = resolve(&scope).map_err(|e| prefixed(&label, e))?;
    check_overlaps(&model.filter_uses()).map_err(|e| prefixed(&label, e))?;
    Ok((deps, model))
}

/// Compiles `.cp` source text without includes. `unit` names the C include guard and spec macros,
/// `source` is quoted in the banner.
pub fn compile_str(src: &str, unit: &str, source: &str) -> Result<Generated> {
    let proto = parse(src)?;
    if let Some((_, ln)) = proto.includes.first() {
        return Err(ErrorSignals::at(*ln, "`#inc=` needs a file on disk; use `compile_file`"));
    }
    let model = resolve(&[(&proto, false)])?;
    check_overlaps(&model.filter_uses())?;
    Ok(Generated {
        rust: rust::generate(&model, source, &Layout::Standalone),
        c: c::generate(&model, source, unit, &Layout::Standalone),
    })
}

/// Compiles a single file into self-contained outputs with its includes inlined.
pub fn compile_file(path: &Path) -> Result<Generated> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let proto = load(path)?;
    for dep in closure(dir, &proto, &path.display().to_string())? {
        resolve_unit(dir, &dep.path, &dep.proto, false)?;
    }
    let (_, model) = resolve_unit(dir, path, &proto, false)?;
    let (unit, name) = (module_of(path)?, file_name(path));
    Ok(Generated {
        rust: rust::generate(&model, name, &Layout::Standalone),
        c: c::generate(&model, name, &unit, &Layout::Standalone),
    })
}

/// Compiles every `.cp` / `.inc.cp` file of a directory as one package. Errors of all files are
/// reported together; filters must not overlap across the whole package.
pub fn compile_package(dir: &Path) -> Result<Package> {
    let files = init::collect_dir(dir).map_err(|e| prefixed(&dir.display().to_string(), e))?;
    let pkg = dir
        .canonicalize()
        .ok()
        .and_then(|p| p.file_name().and_then(|s| s.to_str()).map(str::to_string))
        .ok_or_else(|| ErrorSignals::Generic(format!("{}: cannot determine package name", dir.display())))?;
    let pkg = module_name(pkg, "package")?;

    let (mut units, mut skipped, mut errors) = (Vec::new(), Vec::new(), Vec::<String>::new());
    let mut uses: Vec<(String, FilterIr)> = Vec::new();
    fn push_err(errors: &mut Vec<String>, e: ErrorSignals) {
        let e = e.to_string();
        if !errors.contains(&e) {
            errors.push(e);
        }
    }
    for path in &files {
        let module = match module_of(path) {
            Ok(m) if units.iter().any(|(u, _)| *u == m) => {
                push_err(&mut errors, ErrorSignals::Generic(format!("{}: module `{m}` already exists", path.display())));
                continue;
            }
            Ok(m) => m,
            Err(e) => {
                push_err(&mut errors, prefixed(&path.display().to_string(), e));
                continue;
            }
        };
        let proto = match load(path) {
            Ok(p) => p,
            Err(ErrorSignals::NoError) => {
                skipped.push(file_name(path).to_string());
                continue;
            }
            Err(e) => {
                push_err(&mut errors, e);
                continue;
            }
        };
        let (deps, model) = match resolve_unit(dir, path, &proto, true) {
            Ok(r) => r,
            Err(e) => {
                push_err(&mut errors, e);
                continue;
            }
        };
        let rust_uses: Vec<String> = deps.iter().map(|d| d.module.clone()).collect();
        let c_includes: Vec<String> = proto.includes.iter().map(|(n, _)| format!("{n}_inc")).collect();
        let layout = Layout::Unit { pkg: &pkg, rust_uses: &rust_uses, c_includes: &c_includes };
        let name = file_name(path);
        uses.extend(model.filter_uses().into_iter().map(|(l, f)| (format!("{name}: {l}"), f.clone())));
        units.push((
            module.clone(),
            Generated { rust: rust::generate(&model, name, &layout), c: c::generate(&model, name, &module, &layout) },
        ));
    }
    if !errors.is_empty() {
        return Err(ErrorSignals::Generic(errors.join("\n")));
    }
    let refs: Vec<(String, &FilterIr)> = uses.iter().map(|(l, f)| (l.clone(), f)).collect();
    check_overlaps(&refs)?;

    let modules: Vec<String> = units.iter().map(|(m, _)| m.clone()).collect();
    Ok(Package { mod_rs: rust::package_mod(&pkg, &modules), name: pkg, units, skipped })
}
