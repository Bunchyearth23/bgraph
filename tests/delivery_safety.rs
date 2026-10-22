#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Delivery {
    directory: PathBuf,
    repo: PathBuf,
}

impl Delivery {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "bgraph-delivery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let this = Self {
            repo: directory.join("checkout with spaces"),
            directory,
        };
        fs::create_dir_all(this.directory.join("home")).unwrap();
        for (path, source) in [
            ("export.sh", include_str!("../export.sh")),
            (
                "playground/build.sh",
                include_str!("../playground/build.sh"),
            ),
            ("typst/build.sh", include_str!("../typst/build.sh")),
            (
                "editors/zed/install.sh",
                include_str!("../editors/zed/install.sh"),
            ),
        ] {
            executable(&this.repo.join(path), source);
        }
        write(&this.repo.join("Cargo.toml"), "version = \"1.0.1\"\n");
        for path in [
            "LICENSE",
            "README.md",
            "docs/reference.md",
            "typst/typst.toml",
            "typst/bgraph.typ",
            "typst/README.md",
            "editors/zed/Cargo.toml",
            "editors/zed/README.md",
            "editors/zed/languages/bgraph/config.toml",
            "tree-sitter-bgraph/tree-sitter.json",
            "tree-sitter-bgraph/src/grammar.json",
            "tree-sitter-bgraph/src/node-types.json",
            "tree-sitter-bgraph/src/parser.c",
        ] {
            write(&this.repo.join(path), "original\n");
        }
        write(
            &this.repo.join("editors/zed/extension.toml"),
            "repository = \"file:///old\"\nrev = \"old\"\n",
        );
        executable(
            &this.repo.join("editors/vscode/package.sh"),
            r#"#!/usr/bin/env bash
set -eu
[[ "${BGRAPH_TEST_PACKAGE_FAIL:-0}" != 1 ]] || exit 43
printf fresh-vsix > bgraph-1.0.1.vsix
"#,
        );
        this.tool(
            "cargo",
            r#"#!/usr/bin/env bash
set -eu
printf '%s\n' "$*" >> "$BGRAPH_TEST_BUILD_LOG"
[[ "${BGRAPH_TEST_BUILD_FAIL:-0}" != 1 ]] || exit 42
[[ "$1" != install ]] || exit 0
dir="${CARGO_TARGET_DIR:-$PWD/target}"
target=""
artifact=bgraph.wasm
while (($#)); do
  case "$1" in
    --target-dir) dir="$2"; shift ;;
    --target) target="$2"; shift ;;
    --manifest-path) artifact=zed_bgraph.wasm; shift ;;
  esac
  shift
done
mkdir -p "$dir/$target/release"
for file in "$artifact" bgraph bgraph-lsp bgraph.exe bgraph-lsp.exe; do
  printf 'fresh:%s\n' "$target" > "$dir/$target/release/$file"
done
"#,
        );
        this.tool("rustup", "#!/bin/sh\nprintf '%s\\n' wasm32-unknown-unknown wasm32-wasip2 x86_64-pc-windows-msvc aarch64-pc-windows-msvc\n");
        this.tool(
            "rustc",
            "#!/bin/sh\nprintf 'host: x86_64-unknown-linux-gnu\\n'\n",
        );
        this.tool(
            "tree-sitter",
            r#"#!/usr/bin/env bash
set -eu
[[ "$1" != generate ]] || exit 0
while (($#)); do
  if [[ "$1" == -o ]]; then printf grammar > "$2"; exit; fi
  shift
done
exit 1
"#,
        );
        for tool in ["node", "zip", "cargo-xwin", "bgraph-lsp"] {
            this.tool(tool, "#!/bin/sh\nexit 0\n");
        }
        this
    }

    fn tool(&self, name: &str, source: &str) {
        executable(&self.directory.join("tools").join(name), source);
    }

    fn command(&self, program: &str) -> Command {
        let mut command = Command::new(program);
        command
            .current_dir(&self.repo)
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.directory.join("tools").display(),
                    std::env::var("PATH").unwrap()
                ),
            )
            .env("HOME", self.directory.join("home"))
            .env("BGRAPH_TEST_BUILD_LOG", self.directory.join("build.log"))
            .env("BGRAPH_SKIP_XWIN_INSTALL", "1")
            .env_remove("BGRAPH_WINDOWS_BIN_DIR")
            .env_remove("CARGO_TARGET_DIR")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_COUNT", "2")
            .env("GIT_CONFIG_KEY_0", "commit.gpgSign")
            .env("GIT_CONFIG_VALUE_0", "false")
            .env("GIT_CONFIG_KEY_1", "core.hooksPath")
            .env("GIT_CONFIG_VALUE_1", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@bgraph.invalid")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@bgraph.invalid");
        command
    }

    fn script(&self, path: &str) -> Command {
        let mut command = self.command("bash");
        command.arg(self.repo.join(path));
        command
    }

    fn git(&self, args: &[&str]) -> String {
        let mut command = self.command("git");
        command.args(args);
        String::from_utf8(success(command).stdout).unwrap()
    }

    fn assert_no_staging(&self) {
        assert!(fs::read_dir(&self.repo).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".bgraph-export")
        }));
    }
}

impl Drop for Delivery {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn executable(path: &Path, text: &str) {
    write(path, text);
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn success(mut command: Command) -> Output {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn export_refuses_unsafe_or_unrecognized_destinations_before_building() {
    let delivery = Delivery::new();
    write(&delivery.repo.join("personal/keep.txt"), "keep me");
    symlink(delivery.repo.join("personal"), delivery.repo.join("alias")).unwrap();
    for output in [
        "/",
        ".",
        "export/..",
        "..",
        "personal",
        "alias///",
        "../home",
    ] {
        let result = delivery
            .script("export.sh")
            .args(["--output", output])
            .output()
            .unwrap();
        assert!(!result.status.success(), "accepted {output}");
        assert!(
            !delivery.directory.join("build.log").exists(),
            "built for {output}"
        );
        assert_eq!(
            fs::read_to_string(delivery.repo.join("personal/keep.txt")).unwrap(),
            "keep me"
        );
        assert!(delivery.repo.join("export.sh").exists());
    }
}

#[test]
fn export_preserves_the_previous_export_until_every_build_succeeds() {
    let delivery = Delivery::new();
    let failed = delivery
        .script("export.sh")
        .env("BGRAPH_TEST_BUILD_FAIL", "1")
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert!(!delivery.repo.join("export").exists());
    delivery.assert_no_staging();
    success(delivery.script("export.sh"));
    let output = delivery.repo.join("export");
    write(&output.join("previous.txt"), "previous export");
    for failure in ["BGRAPH_TEST_BUILD_FAIL", "BGRAPH_TEST_PACKAGE_FAIL"] {
        let result = delivery
            .script("export.sh")
            .env(failure, "1")
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert_eq!(
            fs::read_to_string(output.join("previous.txt")).unwrap(),
            "previous export"
        );
        delivery.assert_no_staging();
    }
    success(delivery.script("export.sh"));
    assert!(!output.join("previous.txt").exists());
    // Pre-marker releases still have a recognizable complete layout.
    fs::remove_file(output.join(".bgraph-export")).unwrap();
    success(delivery.script("export.sh"));
    assert!(output.join(".bgraph-export").exists());
    delivery.assert_no_staging();
}

#[test]
fn export_refuses_links_even_to_a_recognized_export() {
    let delivery = Delivery::new();
    success(delivery.script("export.sh"));
    symlink(delivery.repo.join("export"), delivery.repo.join("alias")).unwrap();
    let log = fs::read(delivery.directory.join("build.log")).unwrap();
    for output in ["alias", "alias///", "alias/."] {
        let result = delivery
            .script("export.sh")
            .args(["--output", output])
            .output()
            .unwrap();
        assert!(!result.status.success(), "accepted {output}");
        assert_eq!(fs::read(delivery.directory.join("build.log")).unwrap(), log);
        assert!(delivery.repo.join("export/.bgraph-export").exists());
    }
}

#[test]
fn export_restores_the_previous_directory_when_the_final_rename_fails() {
    let delivery = Delivery::new();
    success(delivery.script("export.sh"));
    write(
        &delivery.repo.join("export/previous.txt"),
        "previous export",
    );
    delivery.tool(
        "mv",
        r#"#!/usr/bin/env bash
set -eu
case "$*" in
  *".bgraph-export."*"/export") exit 44 ;;
esac
exec /bin/mv "$@"
"#,
    );
    let result = delivery.script("export.sh").output().unwrap();
    assert!(!result.status.success());
    assert_eq!(
        fs::read_to_string(delivery.repo.join("export/previous.txt")).unwrap(),
        "previous export"
    );
    delivery.assert_no_staging();
}

#[test]
fn rustup_checks_consume_the_full_target_list_under_pipefail() {
    let delivery = Delivery::new();
    delivery.tool(
        "rustup",
        r#"#!/bin/sh
set -eu
test "$*" = 'target list --installed'
printf '%s\n' wasm32-unknown-unknown wasm32-wasip2 x86_64-pc-windows-msvc aarch64-pc-windows-msvc
# Exceed the pipe buffer so an early reader exit reliably breaks the producer.
seq 1 20000
"#,
    );
    for script in ["playground/build.sh", "typst/build.sh", "export.sh"] {
        success(delivery.script(script));
    }
}

#[test]
fn wasm_builds_copy_from_the_same_target_directory_cargo_uses() {
    let delivery = Delivery::new();
    write(
        &delivery
            .repo
            .join("target/wasm32-unknown-unknown/release/bgraph.wasm"),
        "stale",
    );
    for script in ["playground/build.sh", "typst/build.sh"] {
        for target in [
            PathBuf::from("relative build"),
            delivery.directory.join("absolute build"),
        ] {
            let mut command = delivery.script(script);
            command.env("CARGO_TARGET_DIR", target);
            success(command);
            let artifact = if script.starts_with("playground") {
                "playground/public/bgraph.wasm"
            } else {
                "typst/bgraph.wasm"
            };
            assert_eq!(
                fs::read_to_string(delivery.repo.join(artifact)).unwrap(),
                "fresh:wasm32-unknown-unknown\n"
            );
        }
    }
}

#[test]
fn export_uses_fresh_artifacts_with_a_custom_target_directory_and_another_cwd() {
    let delivery = Delivery::new();
    let mut command = delivery.script("export.sh");
    command
        .env("CARGO_TARGET_DIR", "custom build")
        .current_dir(&delivery.directory);
    success(command);
    for (path, target) in [
        ("linux-x86_64/bin/bgraph", "x86_64-unknown-linux-gnu"),
        ("windows-x86_64/bin/bgraph.exe", "x86_64-pc-windows-msvc"),
        ("windows-aarch64/bin/bgraph.exe", "aarch64-pc-windows-msvc"),
        ("typst/bgraph.wasm", "wasm32-unknown-unknown"),
        ("zed/extension.wasm", "wasm32-wasip2"),
    ] {
        assert_eq!(
            fs::read_to_string(delivery.repo.join("export").join(path)).unwrap(),
            format!("fresh:{target}\n")
        );
    }
}

#[test]
fn export_resolves_prebuilt_windows_files_from_the_callers_directory() {
    let delivery = Delivery::new();
    for target in ["x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"] {
        for binary in ["bgraph.exe", "bgraph-lsp.exe"] {
            write(
                &delivery
                    .directory
                    .join("prebuilt")
                    .join(target)
                    .join(binary),
                "prebuilt windows",
            );
        }
    }
    let mut command = delivery.script("export.sh");
    command
        .current_dir(&delivery.directory)
        .args(["--windows-bin-dir", "prebuilt"]);
    success(command);
    for target in ["windows-x86_64", "windows-aarch64"] {
        assert_eq!(
            fs::read_to_string(
                delivery
                    .repo
                    .join("export")
                    .join(target)
                    .join("bin/bgraph.exe")
            )
            .unwrap(),
            "prebuilt windows"
        );
    }
}

#[test]
fn zed_commits_staged_or_unstaged_grammar_without_committing_other_staged_files() {
    for state in ["unstaged", "staged", "untracked"] {
        let delivery = Delivery::new();
        write(&delivery.repo.join("unrelated.txt"), "original\n");
        delivery.git(&["init", "-q"]);
        delivery.git(&["add", "."]);
        delivery.git(&["commit", "-qm", "initial"]);
        write(&delivery.repo.join("unrelated.txt"), "user's staged work\n");
        delivery.git(&["add", "unrelated.txt"]);
        write(
            &delivery.repo.join("unrelated.txt"),
            "user's later unstaged work\n",
        );
        let before = delivery.git(&["diff", "--cached", "--", "unrelated.txt"]);
        let parser = if state == "untracked" {
            "tree-sitter-bgraph/src/new-parser.c"
        } else {
            "tree-sitter-bgraph/src/parser.c"
        };
        write(&delivery.repo.join(parser), "new parser\n");
        if state == "staged" {
            delivery.git(&["add", "tree-sitter-bgraph"]);
        }
        success(delivery.script("editors/zed/install.sh"));
        assert_eq!(delivery.git(&["show", "HEAD:unrelated.txt"]), "original\n");
        assert_eq!(
            delivery.git(&["diff", "--cached", "--", "unrelated.txt"]),
            before
        );
        assert_eq!(
            fs::read_to_string(delivery.repo.join("unrelated.txt")).unwrap(),
            "user's later unstaged work\n"
        );
        assert_eq!(
            delivery.git(&["show", &format!("HEAD:{parser}")]),
            "new parser\n"
        );
        assert!(
            delivery
                .git(&["diff", "--cached", "--", "tree-sitter-bgraph"])
                .is_empty()
        );
        let revision = delivery.git(&["rev-parse", "HEAD"]);
        assert!(
            fs::read_to_string(delivery.repo.join("editors/zed/extension.toml"))
                .unwrap()
                .contains(revision.trim())
        );
    }
}
