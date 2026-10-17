#![cfg(feature = "simple_tempdir")]

use crate::{BlobStorage, BoxedReader, BufSeekRead, ContentId, Error, LeaseId};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tempfile::TempDir;

pub struct SimpleTempDir {
    root: TempDir,
    refs: Mutex<HashMap<ContentId, usize>>,
}

impl SimpleTempDir {
    pub fn new() -> Result<Self, Error> {
        let root = tempfile::Builder::new()
            .prefix("wezterm-blob-lease-")
            .rand_bytes(8)
            .tempdir()?;
        Ok(Self {
            root,
            refs: Mutex::new(HashMap::new()),
        })
    }

    pub fn new_in<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let path = path.as_ref();
        std::fs::create_dir_all(path)?;
        let root = tempfile::Builder::new()
            .prefix("wezterm-blob-lease-")
            .rand_bytes(8)
            .tempdir_in(path)?;
        Ok(Self {
            root,
            refs: Mutex::new(HashMap::new()),
        })
    }

    fn path_for_content(&self, content_id: ContentId) -> Result<PathBuf, Error> {
        let path = self.root.path().join(format!("{content_id}"));
        std::fs::create_dir_all(path.parent().unwrap())
            .map_err(|err| Error::StorageDirIoError(path.clone(), err))?;
        Ok(path)
    }

    fn add_ref(&self, content_id: ContentId) {
        *self.refs.lock().unwrap().entry(content_id).or_insert(0) += 1;
    }

    fn del_ref(&self, content_id: ContentId) {
        let mut refs = self.refs.lock().unwrap();
        match refs.get_mut(&content_id) {
            Some(count) if *count == 1 => {
                if let Ok(path) = self.path_for_content(content_id) {
                    if let Err(err) = std::fs::remove_file(&path) {
                        eprintln!("Failed to remove {}: {err:#}", path.display());
                    }
                }
                *count = 0;
            }
            Some(count) => {
                *count -= 1;
            }
            None => {
                // Shouldn't really happen...
            }
        }
    }
}

impl BlobStorage for SimpleTempDir {
    fn store(&self, content_id: ContentId, data: &[u8], _lease_id: LeaseId) -> Result<(), Error> {
        let mut refs = self.refs.lock().unwrap();

        // A live reference means that this content-addressed file is already stored, reuse it.
        if let Some(count) = refs.get_mut(&content_id) {
            if *count > 0 {
                *count += 1;
                log::trace!("store: tempfile for content ID {content_id} already exists, reusing");
                return Ok(());
            }
        }

        let path = self.path_for_content(content_id)?;
        let mut file = tempfile::Builder::new()
            .prefix("new-")
            .rand_bytes(5)
            .tempfile_in(&self.root.path())?;

        file.write_all(data)?;
        file.persist(&path)
            .map_err(|persist_err| persist_err.error)?;
        log::trace!("store: tempfile for content ID {content_id} written");

        *refs.entry(content_id).or_insert(0) += 1;

        Ok(())
    }

    fn lease_by_content(&self, content_id: ContentId, _lease_id: LeaseId) -> Result<(), Error> {
        let _refs = self.refs.lock().unwrap();

        let path = self.path_for_content(content_id)?;
        if path.exists() {
            self.add_ref(content_id);
            Ok(())
        } else {
            Err(Error::ContentNotFound(content_id))
        }
    }

    fn get_data(&self, content_id: ContentId, _lease_id: LeaseId) -> Result<Vec<u8>, Error> {
        let _refs = self.refs.lock().unwrap();

        let path = self.path_for_content(content_id)?;
        Ok(std::fs::read(&path).map_err(|err| Error::StorageDirIoError(path, err))?)
    }

    fn get_reader(&self, content_id: ContentId, lease_id: LeaseId) -> Result<BoxedReader, Error> {
        struct Reader {
            file: BufReader<File>,
            content_id: ContentId,
            lease_id: LeaseId,
        }

        impl BufSeekRead for Reader {}

        impl std::io::BufRead for Reader {
            fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
                self.file.fill_buf()
            }
            fn consume(&mut self, amount: usize) {
                self.file.consume(amount)
            }
        }

        impl std::io::Read for Reader {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                self.file.read(buf)
            }
        }

        impl std::io::Seek for Reader {
            fn seek(&mut self, whence: std::io::SeekFrom) -> std::io::Result<u64> {
                self.file.seek(whence)
            }
        }

        impl Drop for Reader {
            fn drop(&mut self) {
                if let Ok(s) = crate::get_storage() {
                    s.advise_lease_dropped(self.lease_id, self.content_id).ok();
                }
            }
        }

        let path = self.path_for_content(content_id)?;
        let file = BufReader::new(std::fs::File::open(&path)?);
        self.add_ref(content_id);

        Ok(Box::new(Reader {
            file,
            content_id,
            lease_id,
        }))
    }

    fn advise_lease_dropped(&self, _lease_id: LeaseId, content_id: ContentId) -> Result<(), Error> {
        self.del_ref(content_id);
        Ok(())
    }

    fn advise_of_pid(&self, _pid: u32) -> Result<(), Error> {
        Ok(())
    }

    fn advise_pid_terminated(&self, _pid: u32) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storing_identical_content_reuses_existing_blob() {
        let storage = SimpleTempDir::new().unwrap();
        let data = b"shared animated image data";
        let content_id = ContentId::for_bytes(data);
        let first_lease = LeaseId::new();
        let second_lease = LeaseId::new();
        let reader_lease = LeaseId::new();

        storage.store(content_id, data, first_lease).unwrap();

        // Keeping this reader open reproduces Windows' access-denied error when
        // store tries to replace the existing content-addressed file.
        let reader = storage.get_reader(content_id, reader_lease).unwrap();
        storage
            .store(content_id, data, second_lease)
            .expect("storing identical content should reuse the existing blob");

        let stored = storage.get_data(content_id, first_lease).unwrap();
        assert_eq!(stored.as_slice(), data);
        assert_eq!(storage.refs.lock().unwrap().get(&content_id), Some(&3));

        let path = storage.path_for_content(content_id).unwrap();
        drop(reader);

        // Reader::drop notifies the process-global storage, while this unit
        // test owns its storage directly, so release that reference here.
        storage
            .advise_lease_dropped(reader_lease, content_id)
            .unwrap();
        storage
            .advise_lease_dropped(first_lease, content_id)
            .unwrap();
        assert!(path.exists());

        storage
            .advise_lease_dropped(second_lease, content_id)
            .unwrap();
        assert!(!path.exists());
    }
}
