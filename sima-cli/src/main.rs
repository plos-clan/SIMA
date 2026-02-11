use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sima_proto::{Request, Response, SOCKET_PATH, decode, encode};
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
    let mut stream =
        UnixStream::connect(SOCKET_PATH).context("Failed to connect to sima-init")?;

    let data = encode(&req).context("Failed to encode request")?;
    stream.write_all(&data).context("Failed to send request")?;
    stream
        .shutdown(Shutdown::Write)
        .context("Failed to shutdown write")?;

    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).context("Failed to read response")?;

    let resp: Response = decode(&buf[..n]).context("Failed to decode response")?;
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
                let pid = svc
                    .pid
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "-".into());
                println!("{:<20} {:>8}  {:>8}", svc.name, status, pid);
            }
        }
    }
}
