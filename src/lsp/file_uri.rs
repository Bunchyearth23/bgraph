use std::path::{Path, PathBuf};

/// Decode a local file URI into a native path, including Windows drives/UNC.
pub(crate) fn path_of(uri: &str) -> Option<PathBuf> {
    decode(uri, cfg!(windows)).map(PathBuf::from)
}

pub(super) fn uri_of(path: &Path) -> String {
    encode(&path.to_string_lossy(), cfg!(windows))
}

fn decode(uri: &str, windows: bool) -> Option<String> {
    if !uri.get(..7)?.eq_ignore_ascii_case("file://") || uri.contains(['?', '#']) {
        return None;
    }
    let rest = &uri[7..];
    let (host, path) = rest.split_once('/')?;
    let mut bytes = Vec::with_capacity(path.len() + 1);
    bytes.push(b'/');
    let mut input = path.bytes();
    while let Some(byte) = input.next() {
        bytes.push(if byte == b'%' {
            let hi = (input.next()? as char).to_digit(16)?;
            let lo = (input.next()? as char).to_digit(16)?;
            (hi * 16 + lo) as u8
        } else {
            byte
        });
    }
    let path = String::from_utf8(bytes).ok()?;
    if path.contains('\0') || host.contains(['%', ':', '@', '\\']) {
        return None;
    }
    let local = host.is_empty() || host.eq_ignore_ascii_case("localhost");
    if !windows {
        return local.then_some(path);
    }
    if !local {
        return Some(format!("\\\\{host}{}", path.replace('/', "\\")));
    }
    let bytes = path.as_bytes();
    if bytes.get(1).is_some_and(u8::is_ascii_alphabetic)
        && bytes.get(2) == Some(&b':')
        && bytes.get(3) == Some(&b'/')
    {
        Some(path[1..].replace('/', "\\"))
    } else {
        None
    }
}

fn encode(path: &str, windows: bool) -> String {
    let path = if windows {
        if let Some(rest) = path.strip_prefix("\\\\?\\UNC\\") {
            format!("//{rest}")
        } else {
            path.strip_prefix("\\\\?\\").unwrap_or(path).into()
        }
        .replace('\\', "/")
    } else {
        path.to_owned()
    };
    let prefix = if windows && path.starts_with("//") {
        "file:"
    } else if path.starts_with('/') {
        "file://"
    } else {
        "file:///"
    };
    let mut uri = prefix.to_owned();
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            uri.push(byte as char);
        } else {
            use std::fmt::Write;
            write!(uri, "%{byte:02X}").unwrap();
        }
    }
    uri
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_file_uris_round_trip_on_every_platform() {
        for (path, uri, windows) in [
            ("/tmp/é #%.bd", "file:///tmp/%C3%A9%20%23%25.bd", false),
            (
                r"C:\work\é #%.bd",
                "file:///C:/work/%C3%A9%20%23%25.bd",
                true,
            ),
            (
                r"\\server\share\a b.bd",
                "file://server/share/a%20b.bd",
                true,
            ),
        ] {
            assert_eq!(encode(path, windows), uri);
            assert_eq!(decode(uri, windows).as_deref(), Some(path));
        }
        assert_eq!(
            decode("file://localhost/tmp/a.bd", false).as_deref(),
            Some("/tmp/a.bd")
        );
        assert_eq!(
            decode("file://localhost/C:/work/a.bd", true).as_deref(),
            Some(r"C:\work\a.bd")
        );
        assert_eq!(encode(r"\\?\C:\work\a.bd", true), "file:///C:/work/a.bd");
        assert_eq!(
            encode(r"\\?\UNC\server\share\a.bd", true),
            "file://server/share/a.bd"
        );
    }

    #[test]
    fn invalid_or_remote_file_uris_are_not_local_paths() {
        for uri in [
            "https://example/a.bd",
            "file://relative",
            "file:///a%",
            "file:///a%GG",
            "file:///a%FF",
            "file:///a%00",
            "file:///a#fragment",
            "file:///a?query",
            "file://user@host/a",
        ] {
            assert_eq!(decode(uri, false), None, "{uri}");
            assert_eq!(decode(uri, true), None, "{uri}");
        }
        assert_eq!(decode("file://server/share/a.bd", false), None);
    }

    #[test]
    fn native_path_and_uri_public_boundary_agree() {
        let path = std::env::temp_dir().join("a #é%.bd");
        let uri = uri_of(&path);
        assert!(uri.starts_with("file:///"));
        assert!(uri.ends_with("a%20%23%C3%A9%25.bd"));
        assert_eq!(path_of(&uri), Some(path));
    }
}
