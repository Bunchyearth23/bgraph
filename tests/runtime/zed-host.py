#!/usr/bin/env python3
"""Run the real Zed host in a disposable Xvfb/profile and capture its LSP wire.

Build inputs first, without installing anything in the user's Zed profile:
  CARGO_TARGET_DIR=target/zed-work cargo build --locked --release \
    --manifest-path editors/zed/Cargo.toml --target wasm32-wasip2
  WASI_SDK_PATH=/usr tree-sitter build --wasm -o /tmp/bgraph-grammar.wasm tree-sitter-bgraph
  python3 tests/runtime/zed-host.py --grammar /tmp/bgraph-grammar.wasm
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import shlex
import signal
import subprocess
import sys
import tempfile
import threading
import time
import tomllib
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ("declared", "overlap", "conflict", "unicode-error", "frame-nested", "size-chain-conflict")
ERROR_FIXTURES = ("conflict", "unicode-error", "size-chain-conflict")
PREVIEW_FILE = os.environ.get("BGRAPH_ZED_PREVIEW_FILE", "preview ' $(touch INJECTED) `touch INJECTED` $file.bd")


def preview_check(base, host):
    """Exercise the language task through its real keybinding, not a copied command."""
    source = base / "project" / PREVIEW_FILE

    def until(check):
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline:
            value = check()
            if value:
                return value
            assert host.poll() is None, "Zed exited during preview verification"
            time.sleep(0.1)
        subprocess.run(["import", "-window", "root", str(base / "preview-failure.png")], timeout=10)
        raise AssertionError(f"Preview timed out; inspect {base}")

    def keys(*values):
        subprocess.run(["xdotool", "key", "--clearmodifiers", *values], check=True, timeout=5)
        time.sleep(0.5)

    def invocations():
        log = base / "preview-commands.jsonl"
        if not log.exists():
            return []
        return [entry for entry in map(json.loads, log.read_text().splitlines())
                if entry["args"] and entry["args"][0] == "watch"]

    windows = subprocess.check_output(["xdotool", "search", "--onlyvisible", "--pid", str(host.pid)],
                                      text=True, timeout=5).splitlines()
    assert windows, "Zed has no visible window"
    names = {window: subprocess.check_output(["xdotool", "getwindowname", window],
                                             text=True, timeout=5).strip() for window in windows}
    (base / "preview-windows.json").write_text(json.dumps(names, indent=2))
    [window] = [window for window, name in names.items() if source.name in name]
    subprocess.run(["xdotool", "windowfocus", "--sync", window], check=True, timeout=5)
    time.sleep(0.5)
    # Append an edit that the task must save before invoking the CLI.
    keys("ctrl+End")
    subprocess.run(["xdotool", "type", "--clearmodifiers", "--", "// UnsavedForPreview\n"],
                   check=True, timeout=5)
    keys("ctrl+shift+v")
    calls = until(invocations)
    assert len(calls) == 1, calls
    assert calls[0]["args"] == ["watch", str(source), "--open", "--port", "0"], calls
    assert "UnsavedForPreview" in source.read_text(), "task did not save the active buffer"
    opened = until(lambda: (base / "preview-open.json").read_text()
                   if (base / "preview-open.json").exists() else None)
    [url] = json.loads(opened)
    assert url.startswith("http://127.0.0.1:") and not url.endswith(":0"), url

    def get(path):
        with urllib.request.urlopen(url + path, timeout=5) as response:
            return response.read().decode()

    assert "PreviewBefore" in get("/svg"), "browser URL does not serve the selected file"
    version = get("/version")
    source.write_text(source.read_text().replace("PreviewBefore", "PreviewAfter"))
    assert get("/version") != version and "PreviewAfter" in get("/svg"), "save did not refresh preview"
    keys("ctrl+shift+v")
    time.sleep(1)
    calls = invocations()
    active = [call for call in calls if Path(f"/proc/{call['pid']}/cmdline").exists()
              and b"bgraph-real" in Path(f"/proc/{call['pid']}/cmdline").read_bytes()]
    (base / "preview-processes.json").write_text(json.dumps({"invocations": calls, "active": active}, indent=2))
    assert len(active) == 1, "repeating the preview task left multiple active watchers"
    assert not (base / "project/INJECTED").exists(), "task interpolated file contents into a shell command"
    [url] = json.loads((base / "preview-open.json").read_text())
    assert get("/svg"), "repeating the task stopped its watcher"
    return {"status": "passed", "task": "bgraph: Preview", "shortcut": "ctrl-shift-v",
            "invocation": calls[0], "url": url, "saved_buffer": True,
            "reload": True, "repeat_watcher_count": len(active), "shell_injection": False}


def proxy(server, wire):
    """Forward actual LSP frames unchanged; record both peers, never synthesize replies."""
    wire = f"{wire}-{os.getpid()}.jsonl"
    with open(wire + ".stderr", "ab") as errors, open(wire, "a", buffering=1) as log:
        child = subprocess.Popen([server], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=errors)
        lock = threading.Lock()

        def forward(source, destination, direction):
            try:
                while True:
                    headers = bytearray()
                    length = None
                    while True:
                        line = source.readline()
                        if not line:
                            return
                        headers.extend(line)
                        if line.lower().startswith(b"content-length:"):
                            length = int(line.split(b":", 1)[1])
                        if line == b"\r\n":
                            break
                    if length is None:
                        raise ValueError("LSP frame without Content-Length")
                    body = bytearray()
                    while len(body) < length:
                        part = source.read(length - len(body))
                        if not part:
                            raise EOFError("incomplete LSP body")
                        body.extend(part)
                    with lock:
                        log.write(json.dumps({"direction": direction, "message": json.loads(body)}) + "\n")
                    destination.write(headers + body)
                    destination.flush()
            except (BrokenPipeError, OSError):
                pass

        outgoing = threading.Thread(target=forward, args=(sys.stdin.buffer, child.stdin, "host"), daemon=True)
        outgoing.start()
        try:
            forward(child.stdout, sys.stdout.buffer, "server")
        finally:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()


def normalize(diagnostics):
    return sorted(diagnostics, key=lambda d: (d["line"], d["column"], d["message"]))


def inside(evidence, zed):
    base = Path(evidence)
    preview = json.loads((base / "inputs.json").read_text()).get("preview", False)
    files = [] if preview else [base / "project" / f"{name}.bd" for name in FIXTURES]
    file_by_uri = {file.as_uri(): file for file in files}
    opened_files = [base / "project", base / "project" / PREVIEW_FILE] if preview else files
    with open(base / "host.stdout", "wb") as stdout, open(base / "host.stderr", "wb") as stderr:
        host = subprocess.Popen([zed, "--user-data-dir", str(base / "profile"), *map(str, opened_files)],
                                stdout=stdout, stderr=stderr)
        try:
            expected = {}
            for name, file in zip(FIXTURES, files):
                checked = subprocess.run([str(base / "bin/bgraph"), "check", str(file), "--json"],
                                         capture_output=True, text=True, timeout=10)
                assert checked.returncode == (1 if name in ERROR_FIXTURES else 0), checked.stderr
                expected[file.as_uri()] = normalize([
                    {"severity": d["severity"], "line": d["line"],
                     "column": d.get("column", d.get("col")), "message": d["message"]}
                    for d in json.loads(checked.stdout)
                ])
                if name in ("declared", "frame-nested"):
                    assert not expected[file.as_uri()], "valid fixture has unexpected diagnostics"
                else:
                    severity = "error" if name in ERROR_FIXTURES else "warning"
                    assert any(d["severity"] == severity for d in expected[file.as_uri()])
                if name == "unicode-error":
                    assert [d["column"] for d in expected[file.as_uri()]] == [25], expected[file.as_uri()]
                if name == "size-chain-conflict":
                    diagnostics = expected[file.as_uri()]
                    assert {10, 11}.issubset({d["line"] for d in diagnostics}), diagnostics
                    for diagnostic in diagnostics:
                        for equality in ("-1*a.width + b.width == 0", "-1*b.width + c.width == 0"):
                            assert equality in diagnostic["message"], diagnostic
            deadline = time.monotonic() + 45
            diagnostics = {}
            opened = set()
            initialize = None
            while time.monotonic() < deadline:
                assert host.poll() is None, "isolated Zed exited before receiving diagnostics"
                for wire in base.glob("lsp-*.jsonl"):
                    for line in wire.read_text().splitlines():
                        try:
                            entry = json.loads(line)
                        except json.JSONDecodeError:
                            continue
                        message = entry["message"]
                        method = message.get("method")
                        if entry["direction"] == "host" and method == "initialize":
                            initialize = message
                        if entry["direction"] == "host" and method == "textDocument/didOpen":
                            opened.add(message["params"]["textDocument"]["uri"])
                        if entry["direction"] == "server" and method == "textDocument/publishDiagnostics":
                            params = message["params"]
                            if params["uri"] not in expected:
                                continue
                            source = file_by_uri[params["uri"]].read_text().splitlines()
                            diagnostics[params["uri"]] = normalize([
                                {"severity": "error" if d["severity"] == 1 else "warning",
                                 "line": d["range"]["start"]["line"] + 1,
                                 "column": len(source[d["range"]["start"]["line"]]
                                               .encode("utf-16-le")[:2*d["range"]["start"]["character"]]
                                               .decode("utf-16-le")) + 1,
                                 "message": d["message"]}
                                for d in params["diagnostics"]
                            ])
                if initialize and set(expected).issubset(opened) and diagnostics == expected:
                    break
                time.sleep(0.1)
            assert initialize, "Zed never initialized the actual bgraph language server"
            assert set(expected).issubset(opened), f"Zed did not open every fixture: {opened}"
            assert diagnostics == expected, f"Zed/CLI diagnostics differ: {diagnostics!r} != {expected!r}"
            assert initialize["params"]["clientInfo"]["name"].lower() == "zed", initialize
            host_log = (base / "profile/logs/Zed.log").read_text().splitlines()
            launches = [line for line in host_log if "starting language server process" in line
                        and f'binary path: "{base}/bin/bgraph-lsp"' in line]
            assert launches, "The extension did not resolve the isolated bgraph-lsp from PATH"
            failures = [line for line in host_log
                        if "ERROR" in line and any(word in line.lower() for word in ("bgraph", "grammar"))]
            assert not failures, f"Zed failed to load the language: {failures}"
            result = json.loads((base / "inputs.json").read_text())
            if preview:
                result["preview"] = preview_check(base, host)
            subprocess.run(["import", "-window", "root", str(base / "host.png")], check=True, timeout=10)
            result.update({"status": "passed", "client": initialize["params"]["clientInfo"],
                           "extension_server_launches": launches,
                           "diagnostics": {file_by_uri[uri].stem: value for uri, value in diagnostics.items()}})
            (base / "result.json").write_text(json.dumps(result, indent=2) + "\n")
            checked = "preview task, shortcut, saved buffer and live reload" if preview else f"diagnostics for {', '.join(FIXTURES)}"
            print(f"PASS Zed host: grammar, extension component, real LSP, {checked}")
            print(f"Evidence: {base}")
        finally:
            host.terminate()
            try:
                host.wait(timeout=5)
            except subprocess.TimeoutExpired:
                host.kill()
                host.wait()


def run(args):
    base = Path(tempfile.mkdtemp(prefix="bgraph-zed-host-"))
    print(f"Isolated Zed evidence directory: {base}", flush=True)
    for name in ("profile/config", "config", "cache", "data", "state", "runtime", "project", "bin"):
        (base / name).mkdir(parents=True, exist_ok=True)
    (base / "runtime").chmod(0o700)
    extension = base / "profile/extensions/installed/bgraph"
    extension.mkdir(parents=True)
    package = ROOT / "editors/zed"
    manifest = tomllib.loads((package / "extension.toml").read_text())
    revision = manifest["grammars"]["bgraph"]["rev"]
    parser = ROOT / "tree-sitter-bgraph/src/parser.c"
    pinned = subprocess.check_output(["git", "show", f"{revision}:tree-sitter-bgraph/src/parser.c"], cwd=ROOT)
    assert pinned == parser.read_bytes(), "grammar source differs from the extension's pinned revision"
    component = Path(args.component).resolve()
    assert component.read_bytes()[:8] == b"\0asm\x0d\0\x01\0", "Zed host needs a wasm component (wasip2)"
    shutil.copyfile(package / "extension.toml", extension / "extension.toml")
    shutil.copytree(package / "languages", extension / "languages")
    shutil.copyfile(component, extension / "extension.wasm")
    (extension / "grammars").mkdir()
    shutil.copyfile(args.grammar, extension / "grammars/bgraph.wasm")
    for name in FIXTURES:
        shutil.copyfile(ROOT / "tests/runtime" / f"{name}.bd", base / "project" / f"{name}.bd")
    for name, source in (("bgraph", args.cli), ("bgraph-lsp-real", args.server)):
        shutil.copy2(source, base / "bin" / name)
    wrapper = base / "bin/bgraph-lsp"
    wrapper.write_text("#!/bin/sh\nexec " + shlex.join([
        sys.executable, str(Path(__file__).resolve()), "--proxy",
        str(base / "bin/bgraph-lsp-real"), str(base / "lsp")]) + "\n")
    wrapper.chmod(0o755)
    if args.preview:
        (base / "project" / PREVIEW_FILE).write_text('node a "PreviewBefore"\n')
        shutil.copyfile(package / "keymap.json", base / "profile/config/keymap.json")
        (base / "bin/bgraph").rename(base / "bin/bgraph-real")
        preview_wrapper = base / "bin/bgraph"
        preview_wrapper.write_text("#!/bin/sh\nexec " + shlex.join([
            sys.executable, str(Path(__file__).resolve()), "--preview-proxy",
            str(base / "bin/bgraph-real"), str(base / "preview-commands.jsonl")]) + ' "$@"\n')
        preview_wrapper.chmod(0o755)
        opener = base / "bin/xdg-open"
        opener.write_text("#!/bin/sh\nexec " + shlex.join([
            sys.executable, "-c", "import json,sys; from pathlib import Path; "
            "Path(sys.argv[1]).write_text(json.dumps(sys.argv[2:]))",
            str(base / "preview-open.json")]) + ' "$@"\n')
        opener.chmod(0o755)
    settings = {
        "telemetry": {"diagnostics": False, "metrics": False}, "auto_update": False,
        "session": {"trust_all_worktrees": True}, "auto_install_extensions": {"html": False},
    }
    if args.preview:
        settings["terminal"] = {"shell": {"program": "/bin/sh"}}
    (base / "profile/config/settings.json").write_text(json.dumps(settings, indent=2))
    # No service directories: this private bus cannot activate the user's keyring or browser portal.
    dbus = base / "dbus.conf"
    dbus.write_text(f'<busconfig><type>session</type><listen>unix:tmpdir={base}/runtime</listen>'
                    '<policy context="default"><allow send_destination="*"/>'
                    '<allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
    artifacts = {"component": component, "grammar": Path(args.grammar), "parser": parser,
                 "cli": base / "bin/bgraph", "server": base / "bin/bgraph-lsp-real",
                 "server_wrapper": wrapper}
    if args.preview:
        artifacts.update({"cli": base / "bin/bgraph-real", "preview_cli_wrapper": base / "bin/bgraph",
                          "preview_task": extension / "languages/bgraph/tasks.json",
                          "preview_keymap": base / "profile/config/keymap.json"})
    (base / "inputs.json").write_text(json.dumps({"grammar_revision": revision, "preview": args.preview,
        "sha256": {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in artifacts.items()}}, indent=2))
    env = os.environ.copy()
    for name in ("WAYLAND_DISPLAY", "GNOME_KEYRING_CONTROL", "SSH_AUTH_SOCK"):
        env.pop(name, None)
    for kind in ("CONFIG", "CACHE", "DATA", "STATE", "RUNTIME"):
        env[f"XDG_{kind}_{'DIR' if kind == 'RUNTIME' else 'HOME'}"] = str(base / kind.lower())
    env.update({"BROWSER": "/usr/bin/false", "ZED_ALLOW_EMULATED_GPU": "1", "ZED_LOG": "info"})
    env["PATH"] = str(base / "bin") + os.pathsep + env.get("PATH", "")
    command = ["xvfb-run", "-a", "-s", "-screen 0 1280x960x24", "dbus-run-session",
               f"--config-file={dbus}", "--", sys.executable, str(Path(__file__).resolve()),
               "--inside", str(base), str(Path(args.zed).resolve())]
    child = subprocess.Popen(command, env=env, start_new_session=True)
    try:
        assert child.wait(timeout=110 if args.preview else 65) == 0, f"Zed host failed; inspect {base}"
    finally:
        try:
            os.killpg(child.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait()


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--proxy":
        proxy(*sys.argv[2:])
    elif len(sys.argv) > 1 and sys.argv[1] == "--inside":
        inside(*sys.argv[2:])
    elif len(sys.argv) > 1 and sys.argv[1] == "--preview-proxy":
        real, log, *arguments = sys.argv[2:]
        with open(log, "a") as output:
            output.write(json.dumps({"pid": os.getpid(), "args": arguments}) + "\n")
        os.execv(real, [real, *arguments])
    else:
        parser = argparse.ArgumentParser(description=__doc__)
        parser.add_argument("--zed", default="/usr/lib/zed/zed-editor")
        parser.add_argument("--cli", default=str(ROOT / "target/debug/bgraph"))
        parser.add_argument("--server", default=str(ROOT / "target/debug/bgraph-lsp"))
        parser.add_argument("--component", default=str(ROOT / "target/zed-work/wasm32-wasip2/release/zed_bgraph.wasm"))
        parser.add_argument("--grammar", required=True)
        parser.add_argument("--preview", action="store_true",
                            help="exercise the bundled preview task, shortcut and watch server")
        run(parser.parse_args())
