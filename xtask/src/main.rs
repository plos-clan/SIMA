use std::env;
use std::process::{Command, Stdio};
use anyhow::{Context, Result};

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
        .args(&["build", "-p", "sima-init"])
        .status()?;
    if !status.success() {
        anyhow::bail!("Failed to build sima-init");
    }

    let script = format!(
        "mount -t tmpfs tmpfs /etc && \
         mount -t tmpfs tmpfs /var/log && \
         mkdir -p /etc/sima.d && \
         cp {sima_src} /etc/sima.yml && \
         cp {shell_src} /etc/sima.d/shell.yml && \
         echo '--- Environment Ready: /etc and /var/log isolated ---' && \
         exec {bin}",
        sima_src = tests_dir.join("sima.yml").display(),
        shell_src = tests_dir.join("shell.yml").display(),
        bin = binary_path.display()
    );

    let status = Command::new("unshare")
        .args(&[
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
