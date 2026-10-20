# Install bgraph

You need the `bgraph` command to follow the first tutorial. An editor
extension is optional. Choose **one** installation route below.

You do not need Rust or Node.js if you have already received a built package.
bgraph is not published on crates.io or the editor extension marketplaces;
use a package supplied to you or the repository you have access to.

## Option A: use a supplied package

1. Extract the archive into a folder you can keep.
2. Choose the folder matching your operating system and processor.
   A full export contains `linux-x86_64/bin`, `windows-x86_64/bin` and
   `windows-aarch64/bin`. An ARM64 Windows computer needs the ARM64 build.
   The current export script does not supply a macOS binary; build from
   source on macOS.
3. Keep both executables from that folder together:
   `bgraph` and `bgraph-lsp` on Linux, or `bgraph.exe` and
   `bgraph-lsp.exe` on Windows.
4. Open a terminal **in that executable folder** and check the program:

### Linux

```sh
./bgraph --version
./bgraph --help
```

### Windows PowerShell

```powershell
.\bgraph.exe --version
.\bgraph.exe --help
```

You should see a version beginning with `bgraph`, followed by command help.
If Linux reports “Permission denied”, make the supplied files executable
with `chmod +x bgraph bgraph-lsp`, then try again.

### Make the command available in any folder

`PATH` is the list of folders your terminal searches for commands. Add the
folder containing the two executables to your user `PATH`, then open a new
terminal:

- **Linux:** you can place the files in `~/.local/bin` if that folder is
  already on your `PATH`. Otherwise add your chosen executable folder using
  your shell's startup configuration.
- **Windows:** search for **Edit environment variables for your account**,
  edit the user **Path**, add the full executable folder, and reopen the
  terminal and your editor.

Check from a different folder:

```sh
bgraph --version
```

If you prefer not to change `PATH`, use the full executable path in place
of `bgraph` in this guide. Quote a path containing spaces. In PowerShell,
a quoted executable path needs `&` before it:

```powershell
& "C:\Tools\bgraph\bin\bgraph.exe" --version
```

The supplied `LICENSE` describes the terms of use.

## Option B: build from the repository

This route requires access to the source repository and **Rust 1.88 or
newer**, including Cargo. Install Rust using the
[official Rust installer](https://www.rust-lang.org/tools/install), then
open a new terminal. The installer explains any platform build-tool
requirements.

Open a terminal in the repository folder: it is the folder containing
`Cargo.toml`, `src` and `docs`. Check your tools:

```sh
rustc --version
cargo --version
```

Install both commands:

```sh
cargo install --locked --path . --bins
```

Cargo normally installs them in `~/.cargo/bin` on Linux/macOS or
`%USERPROFILE%\.cargo\bin` on Windows. Its binary folder must be on `PATH`.
Confirm the installation with `bgraph --version`.

To replace an existing installation after updating the source:

```sh
cargo install --locked --path . --bins --force
```

### Build without installing

From the repository folder:

```sh
cargo build --locked --release
./target/release/bgraph --version
```

In PowerShell, use `.\target\release\bgraph.exe --version`.
The guides use `bgraph` as the command name; replace it with the path to
this local build if you have not installed it.

## Optional: enable PNG output

SVG and PDF are available in the default build. PNG output is optional.
For an installation that includes it:

```sh
cargo install --locked --path . --bins --features png --force
```

Or build it locally:

```sh
cargo build --locked --release --features png
```

If you received a package, its available features depend on how it was built.

## Check that you are ready

Run `bgraph --version` in the terminal you will use for the tutorial.
Once it succeeds, continue to **[Your first diagram](demarrage.md)**.

Later, add [VS Code or Zed support](editeurs.md) for suggestions and preview
shortcuts. Installing an extension alone does not install the command-line
tools.
