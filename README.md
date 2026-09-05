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
├── simactl
├── libsima_proto.so
└── libsima_runtime.so
```

This uses Cargo's native `build.artifact-dir` setting in `.cargo/config.toml` and
requires **nightly Cargo**. The unstable option is enabled in that configuration,
so no additional command-line flags are necessary. Stable Cargo does not support
this automatic artifact export.

The workspace default members select the two shipped binaries and the
`sima-proto` Rust `dylib`. Both programs link to `libsima_proto.so`; request and
response encoding/decoding use concrete functions implemented in that shared
library rather than instantiating the generic codecs in each executable.
The `xtask` helper, build scripts, `.rlib`, `.d`, and other intermediate files
stay out of the artifact directory.

Linux builds prefer dynamic Rust dependencies and use `$ORIGIN` as their runtime
library search path. Rust's matching shared standard library is also required:
`sima-proto/build.rs` copies the already-built `libstd-*.so` from the selected
target's Rust sysroot, changes its ELF SONAME, and exports it as
`libsima_runtime.so`. The linker adapter in `.cargo/sima-linker.sh` makes the
programs and protocol library link against that copy, so their `DT_NEEDED`
entries also name `libsima_runtime.so`. The original toolchain is not modified,
and no `libstd-*.so` alias is needed in the deployment directory. This happens
as part of normal Cargo builds and is repeated if that runtime file is removed.
It is not an `xtask` or a post-build packaging step. The target must provide a
shared Rust standard library, as Linux GNU toolchains do. Builds require Bash,
binutils (`readelf`), and a C compiler driver (`cc`); set `SIMA_LINKER` to use a
different compatible compiler driver. A private runtime copy in Cargo's build
cache also allows `cargo test` and `cargo run` to resolve the renamed library.

Deploy all four files together in the same directory. They can be moved out of
the build tree and run without `LD_LIBRARY_PATH` or an installed Rust toolchain.
System dependencies such as glibc and `libgcc_s` must still be available. For PID 1,
the ELF loader and all required shared libraries must be accessible before
`sima-init` starts; do not place them on a filesystem that SIMA itself must mount.

This is Rust dynamic linking, not a stable C ABI. Build and deploy the executables,
protocol library, and Rust runtime as a matching set using the same toolchain and
dependency versions. Release cross-crate LTO is disabled for this layout. If you
override `RUSTFLAGS`, retain the project's `-C prefer-dynamic` and
`-C link-arg=-Wl,-rpath,$ORIGIN` settings.

Debug and release builds share this export directory; its contents reflect the
last build. Cargo does not remove artifacts from targets that have been deleted
or deselected, so package from a clean artifact directory when changing targets
or toolchains.
Explicit selections such as `--workspace` also export the selected development
targets and should not be used for the distribution build.

The build cache retains Cargo's normal layout. `CARGO_TARGET_DIR` changes the
cache location, not the explicitly configured artifact directory. Override
`CARGO_BUILD_ARTIFACT_DIR` to export somewhere else; both Cargo and the runtime
copy step honor it. Use this environment variable rather than a CLI-only
`--artifact-dir` override, which is not visible to build scripts. If changing the
default path in `.cargo/config.toml`, also update the default in
`sima-proto/build.rs` and the `xtask` runner.

To build and run `sima-init` from the distribution directory in the existing
isolated test environment (requires Linux, `unshare`, and mount support):

```sh
cargo xtask run
cargo xtask run --release
```

The `xtask` helper is only used for this test environment, not distribution builds.

After building, verify the shared-library deployment with:

```sh
cargo test --workspace
cargo build --release
bash tests/shared-library-smoke.sh
```

The smoke test needs Bash, binutils (`readelf`), and permission to create user,
PID, and mount namespaces. It copies the four deployment files to a temporary
directory, clears loader environment overrides, checks the dynamic dependency,
and exercises `simactl` start/stop requests against `sima-init` running as PID 1
in an isolated namespace. It also checks that removing the protocol library
or runtime library prevents the CLI from starting. The host init and service
configuration are not modified. An alternate artifact directory can be passed
as the first argument.

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
