use crate::sshd::session;
use assert_fs::{prelude::*, TempDir};
use rstest::*;
use smol::io::{AsyncReadExt, AsyncWriteExt};
use wezterm_ssh::Session;

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
