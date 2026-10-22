//! Shared file-output safeguards for the CLI and directory builder.
use anyhow::{Context, Result, bail};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

pub fn protect_input(output: &Path, input: &Path) -> Result<()> {
    let same_path = match (fs::canonicalize(output), fs::canonicalize(input)) {
        (Ok(output), Ok(input)) => output == input,
        _ => false,
    };
    #[cfg(unix)]
    let same_file = {
        use std::os::unix::fs::MetadataExt;
        match (fs::metadata(output), fs::metadata(input)) {
            (Ok(output), Ok(input)) => output.dev() == input.dev() && output.ino() == input.ino(),
            _ => false,
        }
    };
    #[cfg(not(unix))]
    let same_file = false;
    if same_path || same_file {
        bail!(
            "refusing to overwrite input {} with an export; choose a different output file",
            input.display()
        );
    }
    Ok(())
}

/// Replace the destination only after the complete export has been written.
/// A fresh file also preserves any other hard links to the old destination.
pub fn write(path: &Path, bytes: impl AsRef<[u8]>) -> Result<()> {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let (temporary, mut file) = loop {
        let temporary = parent.join(format!(
            ".bgraph-export-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("cannot create export beside {}", path.display()));
            }
        }
    };
    let result = (|| {
        match fs::metadata(path) {
            Ok(metadata) => file.set_permissions(metadata.permissions())?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        file.write_all(bytes.as_ref())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        // Windows refuses to remove a temporary that inherited read-only status.
        #[cfg(windows)]
        if let Ok(metadata) = fs::metadata(&temporary) {
            let mut permissions = metadata.permissions();
            permissions.set_readonly(false);
            let _ = fs::set_permissions(&temporary, permissions);
        }
        let _ = fs::remove_file(&temporary);
    }
    result.with_context(|| format!("cannot replace {}", path.display()))
}
