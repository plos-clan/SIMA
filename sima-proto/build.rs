use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const RUNTIME_NAME: &str = "libsima_runtime.so";
const LIBRARY_DIR: &str = "/usr/lib/sima";

fn replace_dynamic_string(
    runtime_bytes: &mut [u8],
    original: &str,
    replacement: &str,
) -> Result<(), Box<dyn Error>> {
    if replacement.len() > original.len() {
        return Err(
            format!("The Rust runtime has insufficient space to replace {original:?}").into(),
        );
    }
    let original_string = format!("{original}\0");
    let offsets: Vec<_> = runtime_bytes
        .windows(original_string.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == original_string.as_bytes()).then_some(offset))
        .collect();
    let [offset] = offsets.as_slice() else {
        return Err(
            format!("Expected one unambiguous {original:?} string in the Rust runtime").into(),
        );
    };
    runtime_bytes[*offset..*offset + original_string.len()].fill(0);
    runtime_bytes[*offset..*offset + replacement.len()].copy_from_slice(replacement.as_bytes());
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=CARGO_BUILD_ARTIFACT_DIR");
    println!("cargo::rerun-if-env-changed=SIMA_LINKER");

    if env::var("CARGO_CFG_TARGET_OS")? != "linux" {
        return Err("SIMA shared-library builds currently require a Linux target".into());
    }

    let output = Command::new(env::var_os("RUSTC").ok_or("RUSTC is not set")?)
        .args(["--print", "target-libdir", "--target", &env::var("TARGET")?])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "Failed to locate the Rust runtime: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let library_dir = PathBuf::from(String::from_utf8(output.stdout)?.trim());
    let mut runtimes = Vec::new();
    for entry in fs::read_dir(&library_dir)? {
        let path = entry?.path();
        if path
            .file_name()
            .and_then(|filename| filename.to_str())
            .is_some_and(|filename| filename.starts_with("libstd-") && filename.ends_with(".so"))
        {
            runtimes.push(path);
        }
    }
    let [runtime] = runtimes.as_slice() else {
        return Err(format!(
            "Expected one shared Rust standard library in {}; use a dynamically linked Linux target",
            library_dir.display()
        )
        .into());
    };

    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").ok_or("Missing manifest directory")?);
    let workspace_root = manifest_dir.parent().ok_or("Missing workspace root")?;
    let artifact_dir = env::var_os("CARGO_BUILD_ARTIFACT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/artifacts"));
    let artifact_dir = workspace_root.join(artifact_dir);
    fs::create_dir_all(&artifact_dir)?;
    let filename = runtime.file_name().ok_or("Runtime has no filename")?;
    let original_name = filename.to_str().ok_or("Runtime filename is not UTF-8")?;
    let dynamic_section = Command::new("readelf")
        .args(["--dynamic", "--wide"])
        .arg(runtime)
        .env("LC_ALL", "C")
        .output()
        .map_err(|error| format!("Failed to run readelf; install binutils: {error}"))?;
    let expected_name = format!("[{original_name}]");
    let dynamic_text = String::from_utf8_lossy(&dynamic_section.stdout);
    if !dynamic_section.status.success()
        || !dynamic_text
            .lines()
            .any(|line| line.contains("(SONAME)") && line.trim_end().ends_with(&expected_name))
    {
        return Err("The Rust runtime does not have the expected ELF SONAME".into());
    }
    let mut runtime_bytes = fs::read(runtime)?;
    replace_dynamic_string(&mut runtime_bytes, original_name, RUNTIME_NAME)?;
    for line in dynamic_text
        .lines()
        .filter(|line| line.contains("(RUNPATH)") || line.contains("(RPATH)"))
    {
        let search_path = line
            .split_once('[')
            .and_then(|(_, value)| value.strip_suffix(']'))
            .ok_or("Failed to read the Rust runtime library search path")?;
        replace_dynamic_string(&mut runtime_bytes, search_path, LIBRARY_DIR)?;
    }

    let private_dir =
        PathBuf::from(env::var_os("OUT_DIR").ok_or("Missing build output directory")?);
    let destination = artifact_dir.join(RUNTIME_NAME);
    for directory in [&private_dir, &artifact_dir] {
        let temporary = directory.join(format!(".{RUNTIME_NAME}.{}", std::process::id()));
        fs::write(&temporary, &runtime_bytes)?;
        fs::set_permissions(&temporary, fs::metadata(runtime)?.permissions())?;
        fs::File::open(&temporary)?
            .set_times(fs::FileTimes::new().set_modified(fs::metadata(runtime)?.modified()?))?;
        fs::rename(&temporary, directory.join(RUNTIME_NAME))?;
    }
    match fs::remove_file(artifact_dir.join(filename)) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    println!("cargo::rerun-if-changed={}", runtime.display());
    println!("cargo::rerun-if-changed={}", destination.display());
    println!(
        "cargo::rerun-if-changed={}",
        private_dir.join(RUNTIME_NAME).display()
    );
    println!("cargo::rustc-link-search=native={}", private_dir.display());
    println!(
        "cargo::rerun-if-changed={}",
        workspace_root.join(".cargo/sima-linker.sh").display()
    );
    Ok(())
}
