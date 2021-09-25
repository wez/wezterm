use crate::sshd::session;
use assert_fs::{prelude::*, TempDir};
use rstest::*;
use smol::io::{AsyncReadExt, AsyncWriteExt};
use std::path::PathBuf;
use wezterm_ssh::Session;

#[rstest]
#[smol_potat::test]
async fn stat_should_retrieve_file_stat(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let file = temp.child("test-file");
    file.touch().unwrap();

    let remote_file = session
        .sftp()
        .open(file.path())
        .await
        .expect("Failed to open remote file");

    let stat = remote_file.stat().await.expect("Failed to read file stat");

    // Verify that file stat makes sense
    assert!(stat.is_file(), "Invalid file stat returned");
}

#[rstest]
#[smol_potat::test]
async fn readdir_should_retrieve_next_dir_entry(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let dir = temp.child("dir");
    dir.create_dir_all().unwrap();
    let file = temp.child("file");
    file.touch().unwrap();
    let link = temp.child("link");
    link.symlink_to_file(file.path()).unwrap();

    let remote_dir = session
        .sftp()
        .opendir(temp.path())
        .await
        .expect("Failed to open remote directory");

    // Collect all of the directory contents (. and .. are included)
    let mut contents = Vec::new();
    while let Ok((path, stat)) = remote_dir.readdir().await {
        let ft = stat.file_type();
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
            (PathBuf::from("."), "dir"),
            (PathBuf::from(".."), "dir"),
            (PathBuf::from("dir"), "dir"),
            (PathBuf::from("file"), "file"),
            (PathBuf::from("link"), "symlink"),
        ]
    );
}

#[rstest]
#[smol_potat::test]
async fn should_support_async_reading(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let file = temp.child("test-file");
    file.write_str("some file contents").unwrap();

    let mut remote_file = session
        .sftp()
        .open(file.path())
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
}

#[rstest]
#[smol_potat::test]
async fn should_support_async_writing(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let file = temp.child("test-file");
    file.write_str("some file contents").unwrap();

    let mut remote_file = session
        .sftp()
        .create(file.path())
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
}

#[rstest]
#[smol_potat::test]
async fn should_support_async_flush(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let file = temp.child("test-file");
    file.write_str("some file contents").unwrap();

    let mut remote_file = session
        .sftp()
        .create(file.path())
        .await
        .expect("Failed to open remote file");

    remote_file.flush().await.expect("Failed to flush file");

    // NOTE: Testing second time to ensure future is properly cleared
    remote_file
        .flush()
        .await
        .expect("Failed to flush file second time");
}

#[rstest]
#[smol_potat::test]
async fn should_support_async_close(#[future] session: Session) {
    let session: Session = session.await;

    let temp = TempDir::new().unwrap();
    let file = temp.child("test-file");
    file.write_str("some file contents").unwrap();

    let mut remote_file = session
        .sftp()
        .create(file.path())
        .await
        .expect("Failed to open remote file");

    remote_file.close().await.expect("Failed to close file");

    // NOTE: Testing second time to ensure future is properly cleared
    remote_file
        .close()
        .await
        .expect("Failed to close file second time");
}
