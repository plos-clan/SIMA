# SIMA

**System Init & Management Agent** - A lightweight, modern init system written in Rust.

Designed to run as PID 1, SIMA manages system services with process isolation, graceful shutdown, and robust zombie reaping.

[Website](https://cpos.plos-clan.org/sima)

## Features

- **PID 1 Init System** - Runs as the system's init process
- **Service Management** - Start, monitor, and manage system services via YAML configuration
- **Process Group Isolation** - Each service runs in its own process group
- **Async Runtime** - Built on Tokio for efficient async I/O and signal handling
- **Graceful Shutdown** - Handles SIGTERM/SIGINT with configurable timeout and force kill fallback
- **Zombie Reaping** - Automatically reaps orphaned child processes
- **Safe Rust** - Written in 100% safe Rust with no unsafe code
- **Structured Logging** - Comprehensive logging with spdlog-rs

## Building

Use the standard Cargo commands from the workspace root:

```sh
cargo build
cargo build --release
```

Cargo copies the final artifacts directly into `target/artifacts/`, including
when compilation is cached. No `xtask` build or post-build command is needed:

```text
target/artifacts/
├── sima-init
└── simactl
```

This uses Cargo's native `build.artifact-dir` setting in `.cargo/config.toml` and
requires **nightly Cargo**. The unstable option is enabled in that configuration,
so no additional command-line flags are necessary. Stable Cargo does not support
this automatic artifact export.

The workspace default members select only the shipped binaries. `sima-proto` is
built as a dependency, while `xtask`, build scripts, `.rlib`, `.d`, and other
intermediate files stay out of the artifact directory. When adding distributable
`dylib`/`cdylib` crates, also add them to `workspace.default-members` so their
shared libraries are exported by the same commands. These libraries do not exist
yet. System libraries and external runtime dependencies are not bundled.

Debug and release builds share this export directory; its contents reflect the
last build. Cargo does not remove artifacts from targets that have been deleted
or deselected, so package from a clean artifact directory when changing targets.
Explicit selections such as `--workspace` also export the selected development
targets and should not be used for the distribution build.

The build cache retains Cargo's normal layout. `CARGO_TARGET_DIR` changes the
cache location, not the explicitly configured artifact directory. Override
`CARGO_BUILD_ARTIFACT_DIR` to export somewhere else.

To build and run `sima-init` from the distribution directory in the existing
isolated test environment (requires Linux, `unshare`, and mount support):

```sh
cargo xtask run
cargo xtask run --release
```

The `xtask` helper is only used for this test environment, not distribution builds.

## Configuration

SIMA loads a main manifest at `/etc/sima.yml` which references service definitions in `/etc/sima.d/`.

**System Manifest** (`/etc/sima.yml`)
```yaml
services:
  - /etc/sima.d/example.yml
  - /etc/sima.d/another-service.yml
```

**Service Definition** (`/etc/sima.d/example.yml`)
```yaml
name: example-service
description: Example service description
cmdline: /usr/bin/example-daemon
```
