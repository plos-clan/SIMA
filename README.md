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
