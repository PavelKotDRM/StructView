//! File writes shared by graphical and command-line operations.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Replace a UTF-8 file only after its complete contents have been written.
///
/// Existing symbolic links are followed, even when their target does not yet
/// exist, and file permissions are retained.
/// A failed write leaves the destination unchanged.
pub fn write_text_atomic(path: &Path, content: &str) -> io::Result<()> {
    let mut target = path.to_path_buf();
    // Resolve the final path component, including dangling links whose target
    // is to be created. Parent-directory links are followed by the OS.
    let mut resolved = false;
    for _ in 0..40 {
        match fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let link = fs::read_link(&target)?;
                target = if link.is_absolute() {
                    link
                } else {
                    target
                        .parent()
                        .filter(|parent| !parent.as_os_str().is_empty())
                        .unwrap_or_else(|| Path::new("."))
                        .join(link)
                };
            }
            Ok(_) => {
                resolved = true;
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                resolved = true;
                break;
            }
            Err(error) => return Err(error),
        }
    }
    if !resolved {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Too many symbolic links",
        ));
    }
    let permissions = match fs::metadata(&target) {
        Ok(metadata) => {
            if metadata.permissions().readonly() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "The file is read-only",
                ));
            }
            Some(metadata.permissions())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(content.as_bytes())?;
    if let Some(permissions) = permissions {
        temporary.as_file().set_permissions(permissions)?;
    }
    temporary.as_file().sync_all()?;
    temporary.persist(&target).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_replaces_files_without_leaving_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("document.json");
        write_text_atomic(&path, "first").unwrap();
        write_text_atomic(&path, "second").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_writes_preserve_the_destination_and_remove_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("document.json");
        fs::write(&path, "original").unwrap();
        let original_permissions = fs::metadata(&path).unwrap().permissions();
        let mut readonly = original_permissions.clone();
        readonly.set_readonly(true);
        fs::set_permissions(&path, readonly).unwrap();
        let result = write_text_atomic(&path, "replacement");
        fs::set_permissions(&path, original_permissions).unwrap();
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read_to_string(&path).unwrap(), "original");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
        assert!(write_text_atomic(directory.path(), "replacement").is_err());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn follows_existing_symbolic_links() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.json");
        let link = directory.path().join("link.json");
        fs::write(&target, "original").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        write_text_atomic(&link, "replacement").unwrap();
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(target).unwrap(), "replacement");
    }

    #[cfg(unix)]
    #[test]
    fn creates_the_target_of_a_dangling_symbolic_link() {
        let directory = tempfile::tempdir_in(".").unwrap();
        let target = directory.path().join("target.json");
        let link = directory.path().join("link.json");
        std::os::unix::fs::symlink("target.json", &link).unwrap();

        write_text_atomic(&link, "created").unwrap();

        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(target).unwrap(), "created");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
    }
}
