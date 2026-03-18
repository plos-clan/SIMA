use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sima_proto::{
    FALLBACK_SOCKET_PATH, PRIMARY_SOCKET_PATH, Request, Response, decode, encode,
    should_fallback_from_connect_error,
};
use std::io::{Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;

#[derive(Parser)]
#[command(name = "simactl", about = "SIMA service manager CLI")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start a service
    Start { name: String },
    /// Stop a service
    Stop { name: String },
    /// Restart a service
    Restart { name: String },
    /// Show status of all services
    Status,
    /// Power off the system
    Poweroff,
    /// Reboot the system
    Reboot,
    /// Soft-reboot (restart userspace only)
    SoftReboot,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let request = match cli.command {
        Command::Start { name } => Request::Start(name),
        Command::Stop { name } => Request::Stop(name),
        Command::Restart { name } => Request::Restart(name),
        Command::Status => Request::Status,
        Command::Poweroff => Request::Poweroff,
        Command::Reboot => Request::Reboot,
        Command::SoftReboot => Request::SoftReboot,
    };

    let response = send_request(request)?;
    print_response(response);
    Ok(())
}

fn send_request(req: Request) -> Result<Response> {
    let mut stream = match UnixStream::connect(PRIMARY_SOCKET_PATH) {
        Ok(stream) => stream,
        Err(primary_err) if should_fallback_from_connect_error(&primary_err) => {
            match UnixStream::connect(FALLBACK_SOCKET_PATH) {
                Ok(stream) => stream,
                Err(fallback_err) => {
                    return Err(fallback_err).with_context(|| {
                        format!(
                            "Failed to connect to sima-init via {} after {} returned {}",
                            FALLBACK_SOCKET_PATH, PRIMARY_SOCKET_PATH, primary_err
                        )
                    });
                }
            }
        }
        Err(err) => {
            return Err(err).with_context(|| {
                format!("Failed to connect to sima-init via {PRIMARY_SOCKET_PATH}")
            });
        }
    };

    let data = encode(&req).context("Failed to encode request")?;
    stream.write_all(&data).context("Failed to send request")?;
    stream
        .shutdown(Shutdown::Write)
        .context("Failed to shutdown write")?;

    let mut buf = Vec::new();
    stream
        .read_to_end(&mut buf)
        .context("Failed to read response")?;

    let resp: Response = decode(&buf).context("Failed to decode response")?;
    Ok(resp)
}

fn print_response(resp: Response) {
    match resp {
        Response::Ok => println!("OK"),
        Response::Error(e) => eprintln!("Error: {}", e),
        Response::StatusReport(services) => {
            if services.is_empty() {
                println!("No services configured.");
                return;
            }
            println!("{:<20} {:>8}  {:>8}", "SERVICE", "STATUS", "PID");
            println!("{}", "-".repeat(40));
            for svc in services {
                let status = if svc.running { "running" } else { "stopped" };
                let pid = svc.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into());
                println!("{:<20} {:>8}  {:>8}", svc.name, status, pid);
            }
        }
    }
}
