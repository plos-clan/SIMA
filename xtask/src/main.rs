use anyhow::{Context, Result};
use std::env;
use std::process::{Command, Stdio};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let task = args.get(1).map(|s| s.as_str());

    match task {
        Some("run") => run_init()?,
        _ => println!("Usage: cargo xtask [run]"),
    }
    Ok(())
}

fn run_init() -> Result<()> {
    let project_root = env::current_dir()?;
    let binary_path = project_root.join("target/debug/sima-init");
    let tests_dir = project_root.join("tests");

    let status = Command::new("cargo")
        .args(["build", "--workspace"])
        .status()?;
    if !status.success() {
        anyhow::bail!("Failed to build sima");
    }

    let script = format!(
        "mount -t tmpfs tmpfs /etc && \
         mount -t tmpfs tmpfs /var/log && \
         mount -t tmpfs tmpfs /run && \
         mkdir -p /etc/sima.d && \
         cp {sima_yml} /etc/sima.yml && \
         cp -r {sima_d}/* /etc/sima.d/ && \
         exec {bin}",
        sima_yml = tests_dir.join("sima.yml").display(),
        sima_d = tests_dir.join("sima.d").display(),
        bin = binary_path.display()
    );

    let status = Command::new("unshare")
        .args([
            "--pid",
            "--mount",
            "--fork",
            "--mount-proc",
            "--map-root-user",
            "bash",
            "-c",
            &script,
        ])
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
