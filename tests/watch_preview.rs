#![cfg(all(feature = "cli", unix, not(target_os = "macos")))]

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

struct Preview {
    directory: PathBuf,
    child: Option<Child>,
}

impl Preview {
    fn new(opener: Option<&str>) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "bgraph-preview-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(directory.join("bin")).unwrap();
        fs::write(directory.join("a diagram '$`.bd"), "node a \"Before\"\n").unwrap();
        if let Some(body) = opener {
            let executable = directory.join("bin/xdg-open");
            fs::write(&executable, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(executable, fs::Permissions::from_mode(0o755)).unwrap();
        }
        Self {
            directory,
            child: None,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_bgraph"));
        command
            .arg("watch")
            .arg(self.directory.join("a diagram '$`.bd"))
            .arg("--open")
            .env("PATH", self.directory.join("bin"))
            .env("BGRAPH_BROWSER_LOG", self.directory.join("browser.log"));
        command
    }

    fn start(&mut self) -> String {
        let child = self
            .command()
            .args(["--port", "0"])
            .stdout(Stdio::from(
                fs::File::create(self.directory.join("stdout")).unwrap(),
            ))
            .stderr(Stdio::from(
                fs::File::create(self.directory.join("stderr")).unwrap(),
            ))
            .spawn()
            .unwrap();
        self.child = Some(child);
        self.until(|| {
            fs::read_to_string(self.directory.join("stdout"))
                .ok()
                .and_then(|text| {
                    text.lines()
                        .find_map(|line| line.trim().strip_prefix("→ ").map(str::to_owned))
                })
        })
    }

    fn until<T>(&self, mut check: impl FnMut() -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(value) = check() {
                return value;
            }
            assert!(
                Instant::now() < deadline,
                "preview timed out: {:?}",
                self.directory
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn request(&self, url: &str, path: &str) -> String {
        let address = url.strip_prefix("http://").unwrap();
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .write_all(
                format!("GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .unwrap();
        let mut reader = BufReader::new(stream);
        let mut length = None;
        loop {
            let mut line = String::new();
            assert_ne!(
                reader.read_line(&mut line).unwrap(),
                0,
                "incomplete HTTP headers"
            );
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length: ") {
                length = Some(value.trim().parse::<usize>().unwrap());
            }
        }
        let mut body = vec![0; length.unwrap()];
        reader.read_exact(&mut body).unwrap();
        String::from_utf8(body).unwrap()
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn preview_opens_its_actual_port_and_reloads_saved_source() {
    let mut preview = Preview::new(Some("printf '%s\\n' \"$@\" > \"$BGRAPH_BROWSER_LOG\""));
    let url = preview.start();
    assert_ne!(url, "http://127.0.0.1:0");
    let opened = preview.until(|| fs::read_to_string(preview.directory.join("browser.log")).ok());
    assert_eq!(opened, format!("{url}\n"));
    assert!(preview.request(&url, "/svg").contains("Before"));
    let version = preview.request(&url, "/version");
    fs::write(
        preview.directory.join("a diagram '$`.bd"),
        "node a \"After\"\n",
    )
    .unwrap();
    assert_ne!(preview.request(&url, "/version"), version);
    assert!(preview.request(&url, "/svg").contains("After"));
    assert!(preview.request(&url, "/").contains("location.reload()"));
}

#[test]
fn occupied_port_fails_before_opening_a_browser() {
    let preview = Preview::new(Some("printf opened > \"$BGRAPH_BROWSER_LOG\""));
    let occupied = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let output = preview
        .command()
        .args(["--port", &occupied.local_addr().unwrap().port().to_string()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot bind"));
    assert!(!preview.directory.join("browser.log").exists());
}

#[test]
fn failed_browser_opening_leaves_a_working_preview_and_visible_url() {
    for opener in [None, Some("exit 7")] {
        let mut preview = Preview::new(opener);
        let url = preview.start();
        preview.until(|| {
            let error = fs::read_to_string(preview.directory.join("stderr")).ok()?;
            (error.contains("cannot open browser") && error.contains(&url)).then_some(())
        });
        assert!(preview.request(&url, "/svg").contains("Before"));
    }
}

#[test]
fn browser_opening_requires_a_single_file() {
    let preview = Preview::new(None);
    let output = Command::new(env!("CARGO_BIN_EXE_bgraph"))
        .arg("watch")
        .arg(&preview.directory)
        .args(["--open", "--once"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("`--open` needs a single"));
}

#[test]
fn zed_preview_task_saves_buffers_and_passes_the_file_as_one_argument() {
    let tasks: serde_json::Value =
        serde_json::from_str(include_str!("../editors/zed/languages/bgraph/tasks.json")).unwrap();
    assert_eq!(tasks[0]["command"], "bgraph");
    assert_eq!(
        tasks[0]["args"],
        serde_json::json!(["watch", "\"$BGRAPH_PREVIEW_FILE\"", "--open", "--port", "0"])
    );
    assert_eq!(tasks[0]["env"]["BGRAPH_PREVIEW_FILE"], "$ZED_FILE");
    assert_eq!(tasks[0]["save"], "current");
    assert_eq!(tasks[0]["allow_concurrent_runs"], false);
    let keymap: serde_json::Value =
        serde_json::from_str(include_str!("../editors/zed/keymap.json")).unwrap();
    assert_eq!(
        keymap[0]["bindings"]["ctrl-shift-v"][1]["task_name"],
        tasks[0]["label"]
    );
}
