use crate::macos::{nsstring, nsstring_to_str};
use crate::ClipboardImage;
use cocoa::appkit::{NSFilenamesPboardType, NSPasteboard, NSStringPboardType};
use cocoa::base::*;
use cocoa::foundation::{NSArray, NSUInteger};
use objc::*;

pub struct Clipboard {
    pasteboard: id,
}

impl Clipboard {
    pub fn new() -> Self {
        let pasteboard = unsafe { NSPasteboard::generalPasteboard(nil) };
        if pasteboard.is_null() {
            panic!("NSPasteboard::generalPasteboard returned null");
        }
        Clipboard { pasteboard }
    }

    /// If the pasteboard holds a list of files (for example, copied from
    /// Finder), return their quoted paths.
    fn filenames(&self) -> Option<String> {
        unsafe {
            let plist = self.pasteboard.propertyListForType(NSFilenamesPboardType);
            if plist.is_null() {
                return None;
            }
            let mut filenames = vec![];
            for i in 0..plist.count() {
                filenames.push(
                    shlex::try_quote(nsstring_to_str(plist.objectAtIndex(i)))
                        .unwrap_or_else(|_| "".into()),
                );
            }
            Some(filenames.join(" "))
        }
    }

    /// If the pasteboard references a local file via a file URL, return its
    /// quoted path.
    fn file_url(&self) -> Option<String> {
        unsafe {
            let s: id = msg_send![self.pasteboard, stringForType: *nsstring("public.file-url")];
            if s.is_null() {
                return None;
            }
            let url = url::Url::parse(nsstring_to_str(s)).ok()?;
            let path = url.to_file_path().ok()?;
            Some(
                shlex::try_quote(&path.to_string_lossy())
                    .unwrap_or_else(|_| "".into())
                    .into_owned(),
            )
        }
    }

    pub fn read(&self) -> anyhow::Result<String> {
        if let Some(files) = self.filenames() {
            return Ok(files);
        }
        if let Some(file) = self.file_url() {
            return Ok(file);
        }
        unsafe {
            let s = self.pasteboard.stringForType(NSStringPboardType);
            if !s.is_null() {
                let str = nsstring_to_str(s);
                return Ok(str.to_string());
            }
        }
        anyhow::bail!("pasteboard read returned empty");
    }

    /// If the pasteboard holds image data and does not reference a local
    /// file, return the encoded image bytes together with any filename
    /// suggested by the source of the image.
    pub fn read_image(&self) -> anyhow::Result<Option<ClipboardImage>> {
        unsafe {
            // A local file reference is preferable: the caller can paste the
            // path of the existing file instead of making a copy of it.
            if !self
                .pasteboard
                .propertyListForType(NSFilenamesPboardType)
                .is_null()
            {
                return Ok(None);
            }
            if self.file_url().is_some() {
                return Ok(None);
            }

            let data = ["public.png", "public.tiff"]
                .iter()
                .map(|ty| {
                    let d: id = msg_send![self.pasteboard, dataForType: *nsstring(ty)];
                    d
                })
                .find(|d| !d.is_null());

            let Some(data) = data else {
                return Ok(None);
            };

            let len: NSUInteger = msg_send![data, length];
            let bytes: *const u8 = msg_send![data, bytes];
            if len == 0 || bytes.is_null() {
                return Ok(None);
            }

            let data = std::slice::from_raw_parts(bytes, len as usize).to_vec();
            Ok(Some(ClipboardImage {
                data,
                suggested_name: self.suggested_name(),
            }))
        }
    }

    /// Try to derive a filename from the URL that the image was copied from.
    fn suggested_name(&self) -> Option<String> {
        unsafe {
            let s: id = msg_send![self.pasteboard, stringForType: *nsstring("public.url")];
            if s.is_null() {
                return None;
            }
            let url = url::Url::parse(nsstring_to_str(s)).ok()?;
            let name = url
                .path_segments()
                .and_then(|mut segments| segments.next_back())
                .filter(|name| !name.is_empty())?;
            Some(name.to_string())
        }
    }

    pub fn write(&mut self, data: String) -> anyhow::Result<()> {
        unsafe {
            self.pasteboard.clearContents();
            let success: BOOL = self
                .pasteboard
                .writeObjects(NSArray::arrayWithObject(nil, *nsstring(&data)));
            anyhow::ensure!(success == YES, "pasteboard write returned false");
            Ok(())
        }
    }
}
