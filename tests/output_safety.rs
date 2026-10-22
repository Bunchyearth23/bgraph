use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Files(PathBuf);

impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "bgraph-output-safety-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn assert_no_temporary(&self) {
        assert!(!fs::read_dir(&self.0).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".bgraph-export-")
        }));
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
#[test]
fn replacing_a_private_export_preserves_its_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let files = Files::new();
    let output = files.0.join("private.svg");
    fs::write(&output, "previous private export").unwrap();
    fs::set_permissions(&output, fs::Permissions::from_mode(0o600)).unwrap();

    bgraph::output::write(&output, "new private export").unwrap();

    assert_eq!(fs::read_to_string(&output).unwrap(), "new private export");
    assert_eq!(
        fs::metadata(&output).unwrap().permissions().mode() & 0o777,
        0o600
    );
    files.assert_no_temporary();
}

#[test]
fn failed_replacement_preserves_the_destination_and_removes_its_temporary() {
    let files = Files::new();
    let output = files.0.join("directory.svg");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("keep"), "original contents").unwrap();
    let permissions = fs::metadata(&output).unwrap().permissions();

    assert!(bgraph::output::write(&output, "new export").is_err());

    assert_eq!(
        fs::read_to_string(output.join("keep")).unwrap(),
        "original contents"
    );
    assert_eq!(fs::metadata(&output).unwrap().permissions(), permissions);
    files.assert_no_temporary();
}

#[cfg(unix)]
#[test]
fn unreadable_destination_metadata_fails_before_content_and_cleans_up() {
    struct Content;
    impl AsRef<[u8]> for Content {
        fn as_ref(&self) -> &[u8] {
            panic!("content must not be written when permissions cannot be read");
        }
    }

    let files = Files::new();
    let output = files.0.join("loop.svg");
    std::os::unix::fs::symlink("loop.svg", &output).unwrap();

    assert!(bgraph::output::write(&output, Content).is_err());

    assert_eq!(fs::read_link(output).unwrap(), PathBuf::from("loop.svg"));
    files.assert_no_temporary();
}

#[cfg(windows)]
#[test]
fn a_read_only_target_does_not_leave_a_read_only_temporary() {
    let files = Files::new();
    let output = files.0.join("private.svg");
    fs::write(&output, "original contents").unwrap();
    let original = fs::metadata(&output).unwrap().permissions();
    let mut read_only = original.clone();
    read_only.set_readonly(true);
    fs::set_permissions(&output, read_only).unwrap();

    let result = bgraph::output::write(&output, "new export");

    fs::set_permissions(&output, original).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read_to_string(output).unwrap(), "original contents");
    files.assert_no_temporary();
}
