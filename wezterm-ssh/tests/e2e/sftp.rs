use crate::sshd::session;
use assert_fs::{prelude::*, TempDir};
use rstest::*;
use ssh2::FileType;
use std::path::PathBuf;
use wezterm_ssh::Session;

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
#[smol_potat::test]
async fn should_support_listing_directory_contents(#[future] session: Session) {
    let session = session.await;

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
        .readdir(temp.path().to_path_buf())
        .await
        .expect("Failed to read directory")
        .into_iter()
        .map(|(p, s)| (p, file_type_to_str(s.file_type())))
        .collect::<Vec<(PathBuf, &'static str)>>();
    contents.sort_unstable_by_key(|(p, _)| p.to_path_buf());

    assert_eq!(
        contents,
        vec![
            (dir1.path().to_path_buf(), "dir"),
            (dir2.path().to_path_buf(), "dir"),
            (file1.path().to_path_buf(), "file"),
            (file2.path().to_path_buf(), "file"),
            (link_dir.path().to_path_buf(), "symlink"),
            (link_file.path().to_path_buf(), "symlink"),
        ]
    );
}
