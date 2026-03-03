use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("builtin_agents.rs");

    let agents_dir = Path::new("../../agents"); // relative to crates/flowforge-agents/

    if !agents_dir.exists() {
        // Generate empty fallback
        let mut f = fs::File::create(&dest_path).unwrap();
        writeln!(
            f,
            "pub fn builtin_agents() -> &'static [(&'static str, &'static str)] {{"
        )
        .unwrap();
        writeln!(f, "    &[]").unwrap();
        writeln!(f, "}}").unwrap();
        return;
    }

    let mut entries = Vec::new();
    collect_md_files(agents_dir, agents_dir, &mut entries);
    entries.sort(); // deterministic ordering

    let mut f = fs::File::create(&dest_path).unwrap();
    writeln!(
        f,
        "pub fn builtin_agents() -> &'static [(&'static str, &'static str)] {{"
    )
    .unwrap();
    writeln!(f, "    &[").unwrap();

    for (rel_path, abs_path) in &entries {
        let abs_str = abs_path.replace('\\', "/"); // normalize for Windows
        writeln!(
            f,
            "        (\"{}\", include_str!(\"{}\")),",
            rel_path, abs_str
        )
        .unwrap();
    }

    writeln!(f, "    ]").unwrap();
    writeln!(f, "}}").unwrap();

    // Tell cargo to re-run if agents dir changes
    println!("cargo:rerun-if-changed=../../agents");
    rerun_if_changed_recursive(agents_dir);
}

fn collect_md_files(base: &Path, dir: &Path, entries: &mut Vec<(String, String)>) {
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_md_files(base, &path, entries);
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                let rel = path.strip_prefix(base).unwrap();
                let rel_name = rel
                    .with_extension("")
                    .display()
                    .to_string()
                    .replace('\\', "/");
                let abs_path = fs::canonicalize(&path).unwrap().display().to_string();
                entries.push((rel_name, abs_path));
            }
        }
    }
}

fn rerun_if_changed_recursive(dir: &Path) {
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                println!("cargo:rerun-if-changed={}", path.display());
                rerun_if_changed_recursive(&path);
            } else {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}
