use anyhow::{bail, Context, Result};
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const USAGE: &str = "Usage: cargo xtask run [--release]";

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let task = args.first().map(String::as_str);

    if matches!(task, None | Some("--help" | "-h")) {
        println!("{USAGE}");
        return Ok(());
    }
    if task != Some("run") {
        bail!("{USAGE}");
    }
    let release = match &args[1..] {
        [] => false,
        [option] if option == "--release" => true,
        _ => bail!("{USAGE}"),
    };

    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("Failed to locate the workspace root")?;
    run_init(project_root, release)
}

fn run_init(project_root: &Path, release: bool) -> Result<()> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = Command::new(cargo);
    command.current_dir(project_root).arg("build");
    if release {
        command.arg("--release");
    }
    if !command
        .status()
        .context("Failed to start Cargo build")?
        .success()
    {
        bail!("Failed to build SIMA");
    }

    let artifact_dir = env::var_os("CARGO_BUILD_ARTIFACT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/artifacts"));
    let binary_path = project_root.join(artifact_dir).join("sima-init");
    let tests_dir = project_root.join("tests");

    let script = "mount -t tmpfs tmpfs /etc && \
         mount -t tmpfs tmpfs /var/log && \
         mount -t tmpfs tmpfs /run && \
         mkdir -p /etc/sima.d && \
         cp \"$1/sima.yml\" /etc/sima.yml && \
         cp -r \"$1/sima.d/.\" /etc/sima.d/ && \
         exec \"$2\"";

    let status = Command::new("unshare")
        .args([
            "--pid",
            "--mount",
            "--fork",
            "--mount-proc",
            "--map-root-user",
            "bash",
            "-c",
            script,
            "sima-init",
        ])
        .arg(tests_dir)
        .arg(binary_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("Failed to start unshare environment")?;

    if !status.success() {
        anyhow::bail!("Unexpected exit status {}", status);
    }

    Ok(())
}
