use crate::sshd::*;
use assert_fs::prelude::*;
use assert_fs::TempDir;
use rstest::*;
use smol::io::{AsyncReadExt, AsyncWriteExt};
use std::convert::TryInto;
use std::path::PathBuf;

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn metadata_should_retrieve_file_stat(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let file = temp.child("test-file");
        file.touch().unwrap();

        let remote_file = session
            .sftp()
            .open(file.path().to_path_buf())
            .await
            .expect("Failed to open remote file");

        let metadata = remote_file
            .metadata()
            .await
            .expect("Failed to read file metadata");

        // Verify that file stat makes sense
        assert!(metadata.is_file(), "Invalid file metadata returned");
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn read_dir_should_retrieve_next_dir_entry(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let dir = temp.child("dir");
        dir.create_dir_all().unwrap();
        let file = temp.child("file");
        file.touch().unwrap();
        let link = temp.child("link");
        link.symlink_to_file(file.path()).unwrap();

        let remote_dir = session
            .sftp()
            .open_dir(temp.path().to_path_buf())
            .await
            .expect("Failed to open remote directory");

        // Collect all of the directory contents (. and .. are included)
        let mut contents = Vec::new();
        while let Ok((path, metadata)) = remote_dir.read_dir().await {
            let ft = metadata.ty;
            contents.push((
                path,
                if ft.is_dir() {
                    "dir"
                } else if ft.is_file() {
                    "file"
                } else {
                    "symlink"
                },
            ));
        }
        contents.sort_unstable_by_key(|(p, _)| p.to_path_buf());

        assert_eq!(
            contents,
            vec![
                (PathBuf::from(".").try_into().unwrap(), "dir"),
                (PathBuf::from("..").try_into().unwrap(), "dir"),
                (PathBuf::from("dir").try_into().unwrap(), "dir"),
                (PathBuf::from("file").try_into().unwrap(), "file"),
                (PathBuf::from("link").try_into().unwrap(), "symlink"),
            ]
        );
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn should_support_async_reading(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let file = temp.child("test-file");
        file.write_str("some file contents").unwrap();

        let mut remote_file = session
            .sftp()
            .open(file.path().to_path_buf())
            .await
            .expect("Failed to open remote file");

        let mut contents = String::new();
        remote_file
            .read_to_string(&mut contents)
            .await
            .expect("Failed to read file to string");

        assert_eq!(contents, "some file contents");

        // NOTE: Testing second time to ensure future is properly cleared
        let mut contents = String::new();
        remote_file
            .read_to_string(&mut contents)
            .await
            .expect("Failed to read file to string second time");
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn should_support_async_writing(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let file = temp.child("test-file");
        file.write_str("some file contents").unwrap();

        let mut remote_file = session
            .sftp()
            .create(file.path().to_path_buf())
            .await
            .expect("Failed to open remote file");

        remote_file
            .write_all(b"new contents for file")
            .await
            .expect("Failed to write to file");

        file.assert("new contents for file");

        // NOTE: Testing second time to ensure future is properly cleared
        remote_file
            .write_all(b"new contents for file")
            .await
            .expect("Failed to write to file second time");
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn should_support_async_flush(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let file = temp.child("test-file");
        file.write_str("some file contents").unwrap();

        let mut remote_file = session
            .sftp()
            .create(file.path().to_path_buf())
            .await
            .expect("Failed to open remote file");

        remote_file.flush().await.expect("Failed to flush file");

        // NOTE: Testing second time to ensure future is properly cleared
        remote_file
            .flush()
            .await
            .expect("Failed to flush file second time");
    })
}

#[rstest]
#[cfg_attr(not(any(target_os = "macos", target_os = "linux")), ignore)]
fn should_support_async_close(#[future] session: SessionWithSshd) {
    if !sshd_available() {
        return;
    }
    smol::block_on(async {
        let session: SessionWithSshd = session.await;

        let temp = TempDir::new().unwrap();
        let file = temp.child("test-file");
        file.write_str("some file contents").unwrap();

        let mut remote_file = session
            .sftp()
            .create(file.path().to_path_buf())
            .await
            .expect("Failed to open remote file");

        remote_file.close().await.expect("Failed to close file");

        // NOTE: Testing second time to ensure future is properly cleared
        remote_file
            .close()
            .await
            .expect("Failed to close file second time");
    })
}
