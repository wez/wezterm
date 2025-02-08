use crate::sshd::*;
use assert_fs::prelude::*;
use assert_fs::TempDir;
use predicates::prelude::*;
use rstest::*;
use std::convert::TryInto;
use wezterm_ssh::{FileType, SftpChannelError, SftpError, Utf8PathBuf};

// Sftp file tests
mod file;

#[inline]
fn file_type_to_str(file_type: FileType) -> &'static str {
    if file_type.is_dir() {
        "dir"
    } else if file_type.is_file() {
        "file"
    } else {
        "symlink"
    }
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn read_dir_should_return_list_of_directories_files_and_symlinks(
    #[future] session: SessionWithSshd,
) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        // $TEMP/dir1/
        // $TEMP/dir2/
        // $TEMP/file1
        // $TEMP/file2
        // $TEMP/dir-link -> $TEMP/dir1/
        // $TEMP/file-link -> $TEMP/file1
        let temp = TempDir::new().unwrap();
        let dir1 = temp.child("dir1");
        dir1.create_dir_all().unwrap();
        let dir2 = temp.child("dir2");
        dir2.create_dir_all().unwrap();
        let file1 = temp.child("file1");
        file1.touch().unwrap();
        let file2 = temp.child("file2");
        file2.touch().unwrap();
        let link_dir = temp.child("link-dir");
        link_dir.symlink_to_dir(dir1.path()).unwrap();
        let link_file = temp.child("link-file");
        link_file.symlink_to_file(file1.path()).unwrap();

        let mut contents = session
            .sftp()
            .read_dir(temp.path().to_path_buf())
            .await
            .expect("Failed to read directory")
            .into_iter()
            .map(|(p, s)| (p, file_type_to_str(s.ty)))
            .collect::<Vec<(Utf8PathBuf, &'static str)>>();
        contents.sort_unstable_by_key(|(p, _)| p.to_path_buf());

        assert_eq!(
            contents,
            vec![
                (dir1.path().to_path_buf().try_into().unwrap(), "dir"),
                (dir2.path().to_path_buf().try_into().unwrap(), "dir"),
                (file1.path().to_path_buf().try_into().unwrap(), "file"),
                (file2.path().to_path_buf().try_into().unwrap(), "file"),
                (link_dir.path().to_path_buf().try_into().unwrap(), "symlink"),
                (
                    link_file.path().to_path_buf().try_into().unwrap(),
                    "symlink"
                ),
            ]
        );
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn create_dir_should_create_a_directory_on_the_remote_filesystem(
    #[future] session: SessionWithSshd,
) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();

        session
            .sftp()
            .create_dir(temp.child("dir").path().to_path_buf(), 0o644)
            .await
            .expect("Failed to create directory");

        // Verify the path exists and is to a directory
        temp.child("dir").assert(predicate::path::is_dir());
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn create_dir_should_return_error_if_unable_to_create_directory(
    #[future] session: SessionWithSshd,
) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();

        // Attempt to create a nested directory structure, which is not supported
        let result = session
            .sftp()
            .create_dir(temp.child("dir").child("dir").path().to_path_buf(), 0o644)
            .await;
        assert!(
            result.is_err(),
            "Unexpectedly succeeded in creating directory: {:?}",
            result
        );

        // Verify the path is not a directory
        temp.child("dir")
            .child("dir")
            .assert(predicate::path::is_dir().not());
        temp.child("dir").assert(predicate::path::is_dir().not());
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn remove_dir_should_remove_a_remote_directory(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();

        // Removing an empty directory should succeed
        let dir = temp.child("dir");
        dir.create_dir_all().unwrap();
        session
            .sftp()
            .remove_dir(dir.path().to_path_buf())
            .await
            .expect("Failed to remove directory");

        // Verify the directory no longer exists
        dir.assert(predicate::path::is_dir().not());
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn remove_dir_should_return_an_error_if_failed_to_remove_directory(
    #[future] session: SessionWithSshd,
) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();

        // Attempt to remove a missing path
        let result = session
            .sftp()
            .remove_dir(temp.child("missing-dir").path().to_path_buf())
            .await;
        assert!(
            result.is_err(),
            "Unexpectedly succeeded in removing missing directory: {:?}",
            result
        );

        // Attempt to remove a non-empty directory
        let dir = temp.child("dir");
        dir.create_dir_all().unwrap();
        dir.child("file").touch().unwrap();

        let result = session.sftp().remove_dir(dir.path().to_path_buf()).await;
        assert!(
            result.is_err(),
            "Unexpectedly succeeded in removing non-empty directory: {:?}",
            result
        );

        // Verify the non-empty directory still exists
        dir.assert(predicate::path::is_dir());

        // Attempt to remove a file (not a directory)
        let file = temp.child("file");
        file.touch().unwrap();
        let result = session.sftp().remove_dir(file.path().to_path_buf()).await;
        assert!(
            result.is_err(),
            "Unexpectedly succeeded in removing file: {:?}",
            result
        );

        // Verify the file still exists
        file.assert(predicate::path::is_file());
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn metadata_should_return_metadata_about_a_file(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let file = temp.child("file");
        file.touch().unwrap();

        let metadata = session
            .sftp()
            .metadata(file.path().to_path_buf())
            .await
            .expect("Failed to get metadata for file");

        // Verify that file metadata makes sense
        assert!(metadata.is_file(), "Invalid file metadata returned");
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn metadata_should_return_metadata_about_a_directory(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let dir = temp.child("dir");
        dir.create_dir_all().unwrap();

        let metadata = session
            .sftp()
            .metadata(dir.path().to_path_buf())
            .await
            .expect("Failed to get metadata for dir");

        // Verify that file metadata makes sense
        assert!(metadata.is_dir(), "Invalid file metadata returned");
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn metadata_should_return_metadata_about_the_file_pointed_to_by_a_symlink(
    #[future] session: SessionWithSshd,
) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();

        let file = temp.child("file");
        file.touch().unwrap();
        let link = temp.child("link");
        link.symlink_to_file(file.path()).unwrap();

        let metadata = session
            .sftp()
            .metadata(link.path().to_path_buf())
            .await
            .expect("Failed to get metadata for symlink");

        // Verify that file metadata makes sense
        assert!(metadata.is_file(), "Invalid file metadata returned");
        assert!(metadata.ty.is_file(), "Invalid file metadata returned");
        assert!(!metadata.ty.is_symlink(), "Invalid file metadata returned");
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn metadata_should_return_metadata_about_the_dir_pointed_to_by_a_symlink(
    #[future] session: SessionWithSshd,
) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();

        let dir = temp.child("dir");
        dir.create_dir_all().unwrap();
        let link = temp.child("link");
        link.symlink_to_dir(dir.path()).unwrap();

        let metadata = session
            .sftp()
            .metadata(link.path().to_path_buf())
            .await
            .expect("Failed to get metadata for symlink");

        // Verify that file metadata makes sense
        assert!(metadata.is_dir(), "Invalid file metadata returned");
        assert!(metadata.ty.is_dir(), "Invalid file metadata returned");
        assert!(!metadata.ty.is_symlink(), "Invalid file metadata returned");
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn metadata_should_fail_if_path_missing(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();

        let result = session
            .sftp()
            .metadata(temp.child("missing").path().to_path_buf())
            .await;
        assert!(
            result.is_err(),
            "Metadata unexpectedly succeeded: {:?}",
            result
        );
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn symlink_metadata_should_return_metadata_about_a_file(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let file = temp.child("file");
        file.touch().unwrap();

        let symlink_metadata = session
            .sftp()
            .symlink_metadata(file.path().to_path_buf())
            .await
            .expect("Failed to get metadata for file");

        // Verify that file metadata makes sense
        assert!(symlink_metadata.is_file(), "Invalid file metadata returned");
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn symlink_metadata_should_return_metadata_about_a_directory(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let dir = temp.child("dir");
        dir.create_dir_all().unwrap();

        let symlink_metadata = session
            .sftp()
            .symlink_metadata(dir.path().to_path_buf())
            .await
            .expect("Failed to metadata for dir");

        // Verify that file metadata makes sense
        assert!(symlink_metadata.is_dir(), "Invalid file metadata returned");
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn symlink_metadata_should_return_metadata_about_symlink_pointing_to_a_file(
    #[future] session: SessionWithSshd,
) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();

        let file = temp.child("file");
        file.touch().unwrap();
        let link = temp.child("link");
        link.symlink_to_file(file.path()).unwrap();

        let metadata = session
            .sftp()
            .symlink_metadata(link.path().to_path_buf())
            .await
            .expect("Failed to get metadata for symlink");

        // Verify that file metadata makes sense
        assert!(!metadata.is_file(), "Invalid file metadata returned");
        assert!(!metadata.ty.is_file(), "Invalid file metadata returned");
        assert!(metadata.ty.is_symlink(), "Invalid file metadata returned");
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn symlink_metadata_should_return_metadata_about_symlink_pointing_to_a_directory(
    #[future] session: SessionWithSshd,
) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();

        let dir = temp.child("dir");
        dir.create_dir_all().unwrap();
        let link = temp.child("link");
        link.symlink_to_dir(dir.path()).unwrap();

        let metadata = session
            .sftp()
            .symlink_metadata(link.path().to_path_buf())
            .await
            .expect("Failed to get metadata for symlink");

        // Verify that file metadata makes sense
        assert!(!metadata.is_dir(), "Invalid file metadata returned");
        assert!(!metadata.ty.is_dir(), "Invalid file metadata returned");
        assert!(metadata.ty.is_symlink(), "Invalid file metadata returned");
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn symlink_metadata_should_fail_if_path_missing(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();

        let result = session
            .sftp()
            .symlink_metadata(temp.child("missing").path().to_path_buf())
            .await;
        assert!(
            result.is_err(),
            "symlink_metadata unexpectedly succeeded: {:?}",
            result
        );
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn symlink_should_create_symlink_pointing_to_file(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let file = temp.child("file");
        file.touch().unwrap();

        let link = temp.child("link");

        session
            .sftp()
            .symlink(file.path().to_path_buf(), link.path().to_path_buf())
            .await
            .expect("Failed to create symlink");

        assert!(
            std::fs::symlink_metadata(link.path())
                .unwrap()
                .file_type()
                .is_symlink(),
            "Symlink is not a symlink!"
        );

        // TODO: This fails even though the type is a symlink:
        //       https://github.com/assert-rs/assert_fs/issues/70
        // link.assert(predicate::path::is_symlink());
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn symlink_should_create_symlink_pointing_to_directory(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let dir = temp.child("dir");
        dir.create_dir_all().unwrap();

        let link = temp.child("link");

        session
            .sftp()
            .symlink(dir.path().to_path_buf(), link.path().to_path_buf())
            .await
            .expect("Failed to create symlink");

        link.assert(predicate::path::is_symlink());
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn symlink_should_succeed_even_if_path_missing(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let file = temp.child("file");

        let link = temp.child("link");

        session
            .sftp()
            .symlink(file.path().to_path_buf(), link.path().to_path_buf())
            .await
            .expect("Failed to create symlink");

        link.assert(predicate::path::is_symlink());
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn read_link_should_return_the_target_of_the_symlink(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();

        // Test a symlink to a directory
        let dir = temp.child("dir");
        dir.create_dir_all().unwrap();
        let link = temp.child("link");
        link.symlink_to_dir(dir.path()).unwrap();

        let path = session
            .sftp()
            .read_link(link.path().to_path_buf())
            .await
            .expect("Failed to read symlink");
        assert_eq!(path, dir.path());

        // Test a symlink to a file
        let file = temp.child("file");
        file.touch().unwrap();
        let link = temp.child("link2");
        link.symlink_to_file(file.path()).unwrap();

        let path = session
            .sftp()
            .read_link(link.path().to_path_buf())
            .await
            .expect("Failed to read symlink");
        assert_eq!(path, file.path());
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn read_link_should_fail_if_path_is_not_a_symlink(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();

        // Test missing path
        let result = session
            .sftp()
            .read_link(temp.child("missing").path().to_path_buf())
            .await;
        assert!(
            result.is_err(),
            "Unexpectedly read link for missing path: {:?}",
            result
        );

        // Test a directory
        let dir = temp.child("dir");
        dir.create_dir_all().unwrap();
        let result = session.sftp().read_link(dir.path().to_path_buf()).await;
        assert!(
            result.is_err(),
            "Unexpectedly read link for directory: {:?}",
            result
        );

        // Test a file
        let file = temp.child("file");
        file.touch().unwrap();
        let result = session.sftp().read_link(file.path().to_path_buf()).await;
        assert!(
            result.is_err(),
            "Unexpectedly read link for file: {:?}",
            result
        );
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn canonicalize_should_resolve_absolute_path_for_relative_path(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        // For resolving parts of a path, all components must exist
        let temp = TempDir::new().unwrap();
        temp.child("hello").create_dir_all().unwrap();
        temp.child("world").touch().unwrap();

        let rel = temp.child(".").child("hello").child("..").child("world");

        // NOTE: Because sftp realpath can still resolve symlinks within a missing path, there
        //       is no guarantee that the resulting path matches the missing path. In fact,
        //       on mac the /tmp dir is a symlink to /private/tmp; so, we cannot successfully
        //       check the accuracy of the path itself, meaning that we can only validate
        //       that the operation was okay.
        let result = session.sftp().canonicalize(rel.path().to_path_buf()).await;
        assert!(
            result.is_ok(),
            "Canonicalize unexpectedly failed: {:?}",
            result
        );
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn canonicalize_should_either_return_resolved_path_or_error_if_missing(
    #[future] session: SessionWithSshd,
) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let missing = temp.child("missing");

        // NOTE: Because sftp realpath can still resolve symlinks within a missing path, there
        //       is no guarantee that the resulting path matches the missing path. In fact,
        //       on mac the /tmp dir is a symlink to /private/tmp; so, we cannot successfully
        //       check the accuracy of the path itself, meaning that we can only validate
        //       that the operation was okay.
        //
        //       Additionally, this has divergent behavior. On some platforms, this returns
        //       the path as is whereas on others this returns a missing path error. We
        //       have to support both checks.
        let result = session
            .sftp()
            .canonicalize(missing.path().to_path_buf())
            .await;
        match result {
            Ok(_) => {}
            Err(SftpChannelError::Sftp(SftpError::NoSuchFile)) => {}
            #[cfg(feature = "libssh-rs")]
            Err(SftpChannelError::LibSsh(libssh_rs::Error::Sftp(_))) => {}
            x => panic!(
                "Unexpected result from canonicalize({}: {:?}",
                missing.path().display(),
                x
            ),
        }
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn canonicalize_should_fail_if_resolving_missing_path_with_dots(
    #[future] session: SessionWithSshd,
) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let missing = temp.child(".").child("hello").child("..").child("world");

        let result = session
            .sftp()
            .canonicalize(missing.path().to_path_buf())
            .await;
        assert!(result.is_err(), "Canonicalize unexpectedly succeeded");
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn rename_should_support_singular_file(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let file = temp.child("file");
        file.write_str("some text").unwrap();

        let dst = temp.child("dst");

        session
            .sftp()
            .rename(
                file.path().to_path_buf(),
                dst.path().to_path_buf(),
                Default::default(),
            )
            .await
            .expect("Failed to rename file");

        // Verify that file was moved to destination
        file.assert(predicate::path::missing());
        dst.assert("some text");
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn rename_should_support_dirtectory(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let dir = temp.child("dir");
        dir.create_dir_all().unwrap();
        let dir_file = dir.child("file");
        dir_file.write_str("some text").unwrap();
        let dir_dir = dir.child("dir");
        dir_dir.create_dir_all().unwrap();

        let dst = temp.child("dst");

        session
            .sftp()
            .rename(
                dir.path().to_path_buf(),
                dst.path().to_path_buf(),
                Default::default(),
            )
            .await
            .expect("Failed to rename directory");

        // Verify that directory was moved to destination
        dir.assert(predicate::path::missing());
        dir_file.assert(predicate::path::missing());
        dir_dir.assert(predicate::path::missing());

        dst.assert(predicate::path::is_dir());
        dst.child("file").assert("some text");
        dst.child("dir").assert(predicate::path::is_dir());
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn rename_should_fail_if_source_path_missing(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let missing = temp.child("missing");
        let dst = temp.child("dst");

        let result = session
            .sftp()
            .rename(
                missing.path().to_path_buf(),
                dst.path().to_path_buf(),
                Default::default(),
            )
            .await;
        assert!(
            result.is_err(),
            "Rename unexpectedly succeeded with missing path: {:?}",
            result
        );
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn remove_file_should_remove_file(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let file = temp.child("file");
        file.touch().unwrap();

        session
            .sftp()
            .remove_file(file.path().to_path_buf())
            .await
            .expect("Failed to remove file");

        file.assert(predicate::path::missing());
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn remove_file_should_remove_symlink_to_file(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let file = temp.child("file");
        file.touch().unwrap();
        let link = temp.child("link");
        link.symlink_to_file(file.path()).unwrap();

        session
            .sftp()
            .remove_file(link.path().to_path_buf())
            .await
            .expect("Failed to remove symlink");

        // Verify link removed but file still exists
        link.assert(predicate::path::missing());
        file.assert(predicate::path::is_file());
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn remove_file_should_remove_symlink_to_directory(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let dir = temp.child("dir");
        dir.create_dir_all().unwrap();
        let link = temp.child("link");
        link.symlink_to_dir(dir.path()).unwrap();

        session
            .sftp()
            .remove_file(link.path().to_path_buf())
            .await
            .expect("Failed to remove symlink");

        // Verify link removed but directory still exists
        link.assert(predicate::path::missing());
        dir.assert(predicate::path::is_dir());
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn remove_file_should_fail_if_path_to_directory(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let dir = temp.child("dir");
        dir.create_dir_all().unwrap();

        let result = session.sftp().remove_file(dir.path().to_path_buf()).await;
        assert!(
            result.is_err(),
            "Unexpectedly removed directory: {:?}",
            result
        );

        // Verify directory still here
        dir.assert(predicate::path::is_dir());
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn remove_file_should_fail_if_path_missing(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();

        let result = session
            .sftp()
            .remove_file(temp.child("missing").path().to_path_buf())
            .await;
        assert!(
            result.is_err(),
            "Unexpectedly removed missing path: {:?}",
            result
        );
    })
}
