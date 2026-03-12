//! Shared `VidaiMock` process harness for integration tests.

use std::error::Error;
use std::io::{self, ErrorKind};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

const MAX_START_ATTEMPTS: usize = 3;

/// Running `VidaiMock` server with automatic teardown on drop.
pub(crate) struct VidaiServer {
    pub base_url: String,
    child: Child,
}

impl Drop for VidaiServer {
    fn drop(&mut self) {
        let _kill_ignored = self.child.kill();
        let _wait_ignored = self.child.wait();
    }
}

/// Starts `VidaiMock` with the provided configuration directory.
///
/// Returns `Ok(None)` when the `vidaimock` binary is unavailable so tests can
/// skip gracefully in environments that do not install the optional tool.
pub(crate) fn spawn_vidaimock(config_dir: &Path) -> Result<Option<VidaiServer>, Box<dyn Error>> {
    if !vidaimock_available() {
        return Ok(None);
    }

    let mut last_error: Option<Box<dyn Error>> = None;
    for _ in 0..MAX_START_ATTEMPTS {
        let port = reserve_port()?;
        let base_url = format!("http://127.0.0.1:{port}");
        let mut command = Command::new("vidaimock");
        command
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--port")
            .arg(port.to_string())
            .arg("--format")
            .arg("openai")
            .arg("--config-dir")
            .arg(config_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let mut child = command.spawn().map_err(|error| {
            io::Error::other(format!("failed to spawn vidaimock process: {error}"))
        })?;

        match wait_for_server(&mut child, base_url.as_str()) {
            Ok(()) => return Ok(Some(VidaiServer { base_url, child })),
            Err(error) => {
                let _kill_ignored = child.kill();
                let _wait_ignored = child.wait();
                last_error = Some(error);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        io::Error::other("vidaimock failed to start for an unknown reason").into()
    }))
}

fn vidaimock_available() -> bool {
    Command::new("vidaimock")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn reserve_port() -> Result<u16, io::Error> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn wait_for_server(child: &mut Child, base_url: &str) -> Result<(), Box<dyn Error>> {
    let metrics_url = format!("{base_url}/metrics");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(250))
        .build()
        .map_err(|error| {
            io::Error::other(format!(
                "failed to build readiness probe HTTP client for vidaimock: {error}"
            ))
        })?;

    for _ in 0..40 {
        if let Some(status) = child.try_wait()? {
            return Err(io::Error::other(format!(
                "vidaimock process exited before readiness probe completed with status {status}"
            ))
            .into());
        }
        if let Ok(response) = client.get(metrics_url.as_str()).send()
            && response.status().is_success()
        {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }

    Err(io::Error::new(
        ErrorKind::TimedOut,
        format!("timed out waiting for vidaimock server readiness at {metrics_url}"),
    )
    .into())
}
