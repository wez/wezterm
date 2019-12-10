use crate::config::FontAttributes;
use crate::font::loader::{FontDataHandle, FontLocator};
use ::font_kit::error::SelectionError;
use ::font_kit::family_handle::FamilyHandle;
use ::font_kit::family_name::FamilyName;
use ::font_kit::handle::Handle;
use ::font_kit::properties::Properties;
use ::font_kit::source::Source;
use ::font_kit::sources::mem::MemSource;
use failure::Fallible;
use std::path::PathBuf;

/// A FontLocator implemented using the font loading
/// functions provided by Source's from font-kit crate.
impl<S> FontLocator for S
where
    S: Source,
{
    fn load_fonts(&self, fonts_selection: &[FontAttributes]) -> Fallible<Vec<FontDataHandle>> {
        let mut handles = vec![];

        for font in fonts_selection {
            let mut props = Properties::new();
            if font.bold {
                props.weight(font_kit::properties::Weight::BOLD);
            }
            if font.italic {
                props.style(font_kit::properties::Style::Italic);
            }
            let family = FamilyName::Title(font.family.clone());
            match self.select_best_match(&[family.clone()], &props) {
                Ok(Handle::Path { path, font_index }) => handles.push(FontDataHandle::OnDisk {
                    path,
                    index: font_index,
                }),
                Ok(Handle::Memory { bytes, font_index }) => handles.push(FontDataHandle::Memory {
                    data: bytes.to_vec(),
                    index: font_index,
                }),
                Err(_) => {}
            }
        }

        Ok(handles)
    }
}

/// A FontLocator that uses a set of fonts discovered in an arbitrary
/// location on the local filesystem
pub struct FileSystemDirectorySource {
    mem_source: MemSource,
}

impl FileSystemDirectorySource {
    pub fn new(paths: &[PathBuf]) -> Self {
        let mut fonts = vec![];

        for path in paths {
            for entry in walkdir::WalkDir::new(path).into_iter() {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => continue,
                };

                let path = entry.path();
                let mut file = match std::fs::File::open(path) {
                    Err(_) => continue,
                    Ok(file) => file,
                };

                use font_kit::file_type::FileType;
                match font_kit::font::Font::analyze_file(&mut file) {
                    Err(_) => continue,
                    Ok(FileType::Single) => fonts.push(Handle::from_path(path.to_owned(), 0)),
                    Ok(FileType::Collection(font_count)) => {
                        for font_index in 0..font_count {
                            fonts.push(Handle::from_path(path.to_owned(), font_index))
                        }
                    }
                }
            }
        }

        Self {
            mem_source: MemSource::from_fonts(fonts.into_iter()).unwrap(),
        }
    }
}

impl Source for FileSystemDirectorySource {
    fn all_fonts(&self) -> Result<Vec<Handle>, SelectionError> {
        self.mem_source.all_fonts()
    }

    fn all_families(&self) -> Result<Vec<String>, SelectionError> {
        self.mem_source.all_families()
    }

    fn select_family_by_name(&self, family_name: &str) -> Result<FamilyHandle, SelectionError> {
        self.mem_source.select_family_by_name(family_name)
    }

    fn select_by_postscript_name(&self, postscript_name: &str) -> Result<Handle, SelectionError> {
        self.mem_source.select_by_postscript_name(postscript_name)
    }
}
