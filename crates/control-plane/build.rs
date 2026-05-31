use std::fs;
use std::path::Path;

fn main() {
    let migrations_dir = Path::new("src/db/migrations");

    // Tell cargo to re-run this script if the migrations directory changes.
    println!("cargo:rerun-if-changed=src/db/migrations");

    let mut files: Vec<String> = fs::read_dir(migrations_dir)
        .expect("failed to read migrations dir")
        .filter_map(|e| {
            let e = e.ok()?;
            let name = e.file_name().into_string().ok()?;
            if name.ends_with(".sql") {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    // Sort so migrations run in filename order (0001_, 0002_, …)
    files.sort();

    let entries: Vec<String> = files
        .iter()
        .map(|file| {
            let key = file.trim_end_matches(".sql");
            format!(
                r#"    ("{key}", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/db/migrations/{file}")))"#
            )
        })
        .collect();

    let generated = format!(
        "pub const MIGRATIONS: &[(&str, &str)] = &[\n{}\n];\n",
        entries.join(",\n")
    );

    let out_dir = std::env::var("OUT_DIR").unwrap();
    fs::write(Path::new(&out_dir).join("migrations.rs"), generated)
        .expect("failed to write migrations.rs");
}
