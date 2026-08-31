//! The `mesh` subcommand: run a mesh server from the command line.
//!
//! `dxcore mesh start [--host <host>] [--port <port>] [--detached]` serves a
//! fresh `MeshService`. With `--detached` it runs in the background, tracked
//! by a pid file, and is stopped with `dxcore mesh stop`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use dxcore::network::mesh::MeshService;
use dxcore::network::servers::HttpServer;

const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_PORT: u16 = 8080;
const PID_FILE: &str = "dxcore-mesh.pid";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(5);

pub fn run(args: &[String]) {
    let Some(command) = args.first() else {
        eprintln!("usage: dxcore mesh <start|stop>");
        return;
    };
    match command.as_str() {
        "start" => start(&args[1..]),
        "stop" => stop(),
        other => {
            eprintln!("unknown mesh command: {other}");
            eprintln!("usage: dxcore mesh <start|stop>");
        }
    }
}

fn start(args: &[String]) {
    let mut host = DEFAULT_HOST.to_string();
    let mut port = DEFAULT_PORT;
    let mut detached = false;
    let mut pid_file: Option<PathBuf> = None;

    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--host" => match it.next() {
                Some(value) => host = value.clone(),
                None => return err("--host requires a value"),
            },
            "--port" => match it.next().and_then(|v| v.parse().ok()) {
                Some(value) => port = value,
                None => return err("--port requires a number"),
            },
            "--detached" => detached = true,
            "--pid-file" => match it.next() {
                Some(value) => pid_file = Some(value.into()),
                None => return err("--pid-file requires a value"),
            },
            other => return err(&format!("unknown flag: {other}")),
        }
    }

    if detached {
        start_detached(&host, port, &pid_file.unwrap_or_else(default_pid_file));
    } else if let Err(e) = serve(&host, port, pid_file.as_deref()) {
        eprintln!("failed to start mesh: {e}");
    }
}

fn serve(host: &str, port: u16, pid_file: Option<&Path>) -> Result<(), Box<dyn std::error::Error>> {
    let mesh = Arc::new(MeshService::new());
    let server = HttpServer::bind((host, port), mesh)?;
    if let Some(path) = pid_file {
        std::fs::write(path, std::process::id().to_string())?;
    }
    println!("mesh listening on http://{}", server.addr());
    server.serve()?;
    Ok(())
}

fn start_detached(host: &str, port: u16, pid_file: &Path) {
    let _ = std::fs::remove_file(pid_file);
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => return err(&format!("cannot locate binary: {e}")),
    };
    let mut child = match std::process::Command::new(exe)
        .args([
            "mesh",
            "start",
            "--host",
            host,
            "--port",
            &port.to_string(),
            "--pid-file",
        ])
        .arg(pid_file)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => return err(&format!("failed to spawn detached mesh: {e}")),
    };

    // The child writes its pid once the server is bound. Wait for it, or
    // for the child to give up.
    let deadline = std::time::Instant::now() + STARTUP_TIMEOUT;
    while std::time::Instant::now() < deadline {
        if pid_file.exists() {
            let pid = std::fs::read_to_string(pid_file).unwrap_or_default();
            println!("mesh started detached (pid {}) at http://{host}:{port}", pid.trim());
            return;
        }
        if let Ok(Some(_)) = child.try_wait() {
            return err("detached mesh exited during startup");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    err("timed out waiting for detached mesh to start");
}

fn stop() {
    let pid_file = default_pid_file();
    let pid_text = match std::fs::read_to_string(&pid_file) {
        Ok(text) => text,
        Err(_) => {
            eprintln!("no detached mesh running ({} not found)", pid_file.display());
            return;
        }
    };
    let pid: i32 = match pid_text.trim().parse() {
        Ok(pid) => pid,
        Err(_) => {
            eprintln!("corrupt pid file: {}", pid_file.display());
            return;
        }
    };

    let status = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status();
    if let Ok(status) = status {
        if status.success() {
            for _ in 0..50 {
                if !process_alive(pid) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            let _ = std::fs::remove_file(&pid_file);
            println!("stopped mesh (pid {pid})");
            return;
        }
    }
    if !process_alive(pid) {
        let _ = std::fs::remove_file(&pid_file);
        println!("mesh (pid {pid}) was already stopped");
        return;
    }
    eprintln!("failed to stop mesh (pid {pid})");
}

fn process_alive(pid: i32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn default_pid_file() -> PathBuf {
    std::env::temp_dir().join(PID_FILE)
}

fn err(message: &str) {
    eprintln!("{message}");
    eprintln!("usage: dxcore mesh start [--host <host>] [--port <port>] [--detached]");
}
