//! The `mesh` CLI subcommand, exercised against the real binary.

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

fn pid_file() -> PathBuf {
    std::env::temp_dir().join("dxlib-mesh.pid")
}

fn process_alive(pid: i32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[test]
fn mesh_start_stop_detached() {
    let _ = std::fs::remove_file(pid_file());
    let bin = env!("CARGO_BIN_EXE_dxlib");

    let status = Command::new(bin)
        .args(["mesh", "start", "--host", "127.0.0.1", "--port", "0", "--detached"])
        .status()
        .unwrap();
    assert!(status.success());

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let pid = loop {
        if let Ok(text) = std::fs::read_to_string(pid_file()) {
            break text.trim().parse::<i32>().unwrap();
        }
        assert!(std::time::Instant::now() < deadline, "mesh did not start");
        std::thread::sleep(Duration::from_millis(100));
    };

    assert!(process_alive(pid));

    let status = Command::new(bin).args(["mesh", "stop"]).status().unwrap();
    assert!(status.success());
    assert!(!pid_file().exists());
    assert!(!process_alive(pid));
}
