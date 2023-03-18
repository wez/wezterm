use crate::os::x11::xcb_util::*;
use crate::x11::XConnection;
use crate::MouseCursor;
use anyhow::{ensure, Context};
use config::ConfigHandle;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::ffi::OsStr;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use xcb::x::Cursor;
use xcb::Xid;

// X11 classic Cursor glyphs
pub const HAND1: u16 = 58;
pub const SB_H_DOUBLE_ARROW: u16 = 108;
pub const SB_V_DOUBLE_ARROW: u16 = 116;
pub const TOP_LEFT_ARROW: u16 = 132;
pub const TOP_LEFT_CORNER: u16 = 134;
pub const XTERM: u16 = 152;

pub struct XcbCursor {
    pub id: Cursor,
    pub conn: Weak<XConnection>,
}

impl Drop for XcbCursor {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.upgrade() {
            conn.send_request_no_reply_log(&xcb::x::FreeCursor { cursor: self.id });
        }
    }
}

pub struct CursorInfo {
    cursors: HashMap<Option<MouseCursor>, XcbCursor>,
    cursor: Option<MouseCursor>,
    conn: Weak<XConnection>,
    size: Option<u32>,
    theme: Option<String>,
    icon_path: Vec<PathBuf>,
    pict_format_id: Option<xcb::render::Pictformat>,
}

fn icon_path() -> Vec<PathBuf> {
    let path = match std::env::var_os("XCURSOR_PATH") {
        Some(path) => {
            log::trace!("Using $XCURSOR_PATH icon path: {:?}", path);
            path
        }
        None => {
            log::trace!("Constructing default icon path because $XCURSOR_PATH is not set");

            fn add_icons_dir(path: &OsStr, dest: &mut Vec<PathBuf>) {
                for entry in std::env::split_paths(path) {
                    dest.push(entry.join("icons"));
                }
            }

            fn xdg_location(name: &str, def: &str, dest: &mut Vec<PathBuf>) {
                if let Some(var) = std::env::var_os(name) {
                    log::trace!("Using ${} location {:?}", name, var);
                    add_icons_dir(&var, dest);
                } else {
                    log::trace!("Using {} because ${} is not set", def, name);
                    add_icons_dir(OsStr::new(def), dest);
                }
            }

            let mut path = vec![];
            xdg_location("XDG_DATA_HOME", "~/.local/share", &mut path);
            path.push("~/.icons".into());
            xdg_location("XDG_DATA_DIRS", "/usr/local/share:/usr/share", &mut path);
            path.push("/usr/share/pixmaps".into());
            path.push("~/.cursors".into());
            path.push("/usr/share/cursors/xorg-x11".into());
            path.push("/usr/X11R6/lib/X11/icons".into());

            std::env::join_paths(path).expect("failed to compose default xcursor path")
        }
    };

    fn tilde_expand(p: PathBuf) -> PathBuf {
        match p.to_str() {
            Some(s) => {
                if s.starts_with("~/") {
                    if let Some(home) = dirs_next::home_dir() {
                        home.join(&s[2..])
                    } else {
                        p.into()
                    }
                } else {
                    p.into()
                }
            }
            None => p.into(),
        }
    }

    std::env::split_paths(&path).map(tilde_expand).collect()
}

fn cursor_size(xcursor_size: &Option<u32>, map: &HashMap<String, String>) -> u32 {
    if let Some(size) = xcursor_size {
        return *size;
    }

    if let Ok(size) = std::env::var("XCURSOR_SIZE") {
        if let Ok(size) = size.parse::<u32>() {
            return size;
        }
    }

    if let Some(size) = map.get("Xcursor.size") {
        if let Ok(size) = size.parse::<u32>() {
            return size;
        }
    }

    if let Some(dpi) = map.get("Xft.dpi") {
        if let Ok(dpi) = dpi.parse::<u32>() {
            return dpi * 16 / 72;
        }
    }

    // Probably a good default?
    24
}

impl CursorInfo {
    pub fn new(config: &ConfigHandle, conn: &Rc<XConnection>) -> Self {
        let mut size = None;
        let mut theme = None;
        let mut pict_format_id = None;
        // If we know the theme to use, then we need the render extension
        // if we are to be able to load the cursor
        let has_render = conn
            .active_extensions()
            .any(|e| e == xcb::Extension::Render);
        if has_render {
            if let Ok(vers) = conn.send_and_wait_request(&xcb::render::QueryVersion {
                client_major_version: xcb::render::MAJOR_VERSION,
                client_minor_version: xcb::render::MINOR_VERSION,
            }) {
                // 0.5 and later have the required support
                if (vers.major_version(), vers.minor_version()) >= (0, 5) {
                    size.replace(cursor_size(&config.xcursor_size, &*conn.xrm.borrow()));
                    theme = config
                        .xcursor_theme
                        .as_ref()
                        .map(|s| s.to_string())
                        .or_else(|| conn.xrm.borrow().get("Xcursor.theme").cloned());

                    // Locate the Pictformat corresponding to ARGB32
                    if let Ok(formats) =
                        conn.send_and_wait_request(&xcb::render::QueryPictFormats {})
                    {
                        for fmt in formats.formats() {
                            if fmt.depth() == 32 {
                                let direct = fmt.direct();
                                if direct.alpha_shift == 24
                                    && direct.red_shift == 16
                                    && direct.green_shift == 8
                                    && direct.blue_shift == 0
                                {
                                    pict_format_id.replace(fmt.id());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        let icon_path = icon_path();
        log::trace!("icon_path is {:?}", icon_path);

        Self {
            cursors: HashMap::new(),
            cursor: None,
            conn: Rc::downgrade(conn),
            size,
            theme,
            icon_path,
            pict_format_id,
        }
    }

    fn conn(&self) -> Rc<XConnection> {
        self.conn.upgrade().expect("XConnection to be alive")
    }

    pub fn set_cursor(
        &mut self,
        window_id: xcb::x::Window,
        cursor: Option<MouseCursor>,
    ) -> anyhow::Result<()> {
        if cursor == self.cursor {
            return Ok(());
        }

        let conn = self.conn();

        let cursor_id = match self.cursors.get(&cursor) {
            Some(cursor) => cursor.id,
            None => match self.load_themed(&conn, cursor) {
                Some(c) => c,
                None => self.load_basic(&conn, cursor),
            },
        };

        conn.send_request_no_reply(&xcb::x::ChangeWindowAttributes {
            window: window_id,
            value_list: &[xcb::x::Cw::Cursor(cursor_id)],
        })
        .context("set_cursor")?;

        self.cursor = cursor;

        Ok(())
    }

    fn create_blank(&mut self, conn: &Rc<XConnection>) -> anyhow::Result<Cursor> {
        let mut pixels = [0u8; 4];

        let image = XcbImage::create_native(
            conn,
            1,
            1,
            xcb::x::ImageFormat::ZPixmap as u32,
            32,
            std::ptr::null_mut(),
            pixels.len() as u32,
            pixels.as_mut_ptr(),
        )?;

        let pixmap = conn.generate_id();
        conn.send_request_no_reply(&xcb::x::CreatePixmap {
            depth: 32,
            pid: pixmap,
            drawable: xcb::x::Drawable::Window(conn.root),
            width: 1,
            height: 1,
        })
        .context("CreatePixmap")?;

        let gc = conn.generate_id();
        conn.send_request_no_reply(&xcb::x::CreateGc {
            cid: gc,
            drawable: xcb::x::Drawable::Pixmap(pixmap),
            value_list: &[],
        })
        .context("CreateGc")?;

        image.put(conn, pixmap.resource_id(), gc.resource_id(), 0, 0, 0);

        conn.send_request(&xcb::x::FreeGc { gc });

        let pic = conn.generate_id();
        conn.send_request_no_reply(&xcb::render::CreatePicture {
            pid: pic,
            drawable: xcb::x::Drawable::Pixmap(pixmap),
            format: self.pict_format_id.unwrap(),
            value_list: &[],
        })
        .context("create_picture")?;

        conn.send_request(&xcb::x::FreePixmap { pixmap });

        let cursor_id: Cursor = conn.generate_id();
        conn.send_request_no_reply(&xcb::render::CreateCursor {
            cid: cursor_id,
            source: pic,
            x: 0,
            y: 0,
        })
        .context("create_cursor")?;

        conn.send_request_no_reply(&xcb::render::FreePicture { picture: pic })
            .context("FreePicture")?;

        Ok(cursor_id)
    }

    fn load_themed(
        &mut self,
        conn: &Rc<XConnection>,
        cursor: Option<MouseCursor>,
    ) -> Option<Cursor> {
        if cursor.is_none() {
            match self.create_blank(conn) {
                Ok(cursor_id) => {
                    self.cursors.insert(
                        cursor,
                        XcbCursor {
                            id: cursor_id,
                            conn: Rc::downgrade(&conn),
                        },
                    );
                    return Some(cursor_id);
                }
                Err(err) => {
                    log::error!("Failed to create blank cursor: {:#}", err);
                    return self.load_themed(conn, Some(MouseCursor::Arrow));
                }
            }
        }

        let theme = self.theme.as_deref().unwrap_or("default");
        self.pict_format_id?;

        let names: &[&str] = match cursor.unwrap_or(MouseCursor::Arrow) {
            MouseCursor::Arrow => &["top_left_arrow", "left_ptr"],
            MouseCursor::Hand => &["hand2"],
            MouseCursor::Text => &["xterm"],
            MouseCursor::SizeUpDown => &["sb_v_double_arrow"],
            MouseCursor::SizeLeftRight => &["sb_h_double_arrow"],
        };

        let mut theme_list = vec![theme.to_string()];
        let mut visited = HashSet::new();

        while !theme_list.is_empty() {
            let theme = theme_list.remove(0);
            if visited.contains(&theme) {
                continue;
            }

            visited.insert(theme.clone());

            for dir in &self.icon_path {
                for name in names {
                    let candidate = dir.join(&theme).join("cursors").join(name);
                    log::trace!(
                        "candidate for theme={theme} {:?} is {:?}",
                        cursor,
                        candidate
                    );
                    if let Ok(file) = std::fs::File::open(&candidate) {
                        match self.parse_cursor_file(conn, file) {
                            Ok(cursor_id) => {
                                self.cursors.insert(
                                    cursor,
                                    XcbCursor {
                                        id: cursor_id,
                                        conn: Rc::downgrade(&conn),
                                    },
                                );

                                log::trace!("{:?} resolved to {:?}", cursor, candidate);
                                return Some(cursor_id);
                            }
                            Err(err) => log::error!("{:#}", err),
                        }
                    }
                }

                let theme_index = dir.join(&theme).join("index.theme");
                if let Some(inherited) = extract_inherited_theme_name(theme_index) {
                    log::trace!("theme {theme} inherits from theme {inherited}");
                    theme_list.push(inherited);
                }
            }
        }
        None
    }

    fn load_basic(&mut self, conn: &Rc<XConnection>, cursor: Option<MouseCursor>) -> Cursor {
        let id_no = match cursor.unwrap_or(MouseCursor::Arrow) {
            // `/usr/include/X11/cursorfont.h`
            // <https://docs.rs/xcb-util/0.3.0/src/xcb_util/cursor.rs.html>
            MouseCursor::Arrow => TOP_LEFT_ARROW,
            MouseCursor::Hand => HAND1,
            MouseCursor::Text => XTERM,
            MouseCursor::SizeUpDown => SB_V_DOUBLE_ARROW,
            MouseCursor::SizeLeftRight => SB_H_DOUBLE_ARROW,
        };
        log::trace!("loading X11 basic cursor {} for {:?}", id_no, cursor);

        let cursor_id: Cursor = conn.generate_id();
        conn.send_request_no_reply_log(&xcb::x::CreateGlyphCursor {
            cid: cursor_id,
            source_font: conn.cursor_font_id,
            mask_font: conn.cursor_font_id,
            source_char: id_no,
            mask_char: id_no + 1,
            fore_red: 0xffff,
            fore_green: 0xffff,
            fore_blue: 0xffff,
            back_red: 0,
            back_green: 0,
            back_blue: 0,
        });

        self.cursors.insert(
            cursor,
            XcbCursor {
                id: cursor_id,
                conn: Rc::downgrade(&conn),
            },
        );

        cursor_id
    }

    fn parse_cursor_file(
        &self,
        conn: &Rc<XConnection>,
        mut file: std::fs::File,
    ) -> anyhow::Result<Cursor> {
        /* See: <https://cgit.freedesktop.org/xcb/util-cursor/tree/cursor/load_cursor.c>
         *
         * Cursor files start with a header.  The header
         * contains a magic number, a version number and a
         * table of contents which has type and offset information
         * for the remaining tables in the file.
         *
         * File minor versions increment for compatible changes
         * File major versions increment for incompatible changes (never, we hope)
         *
         * Chunks of the same type are always upward compatible.  Incompatible
         * changes are made with new chunk types; the old data can remain under
         * the old type.  Upward compatible changes can add header data as the
         * header lengths are specified in the file.
         *
         *  File:
         *      FileHeader
         *      LISTofChunk
         *
         *  FileHeader:
         *      CARD32          magic       magic number
         *      CARD32          header      bytes in file header
         *      CARD32          version     file version
         *      CARD32          ntoc        number of toc entries
         *      LISTofFileToc   toc         table of contents
         *
         *  FileToc:
         *      CARD32          type        entry type
         *      CARD32          subtype     entry subtype (size for images)
         *      CARD32          position    absolute file position
         */

        #[derive(Debug)]
        struct FileHeader {
            magic: u32,
            _header: u32,
            _version: u32,
            ntoc: u32,
        }
        const MAGIC: u32 = 0x72756358;
        const IMAGE_TYPE: u32 = 0xfffd0002;

        #[derive(Debug)]
        struct Toc {
            type_: u32,
            subtype: u32,
            position: u32,
        }

        /// Read a u32 that is stored in little endian format,
        /// return in host byte order
        fn read_u32(r: &mut dyn Read) -> anyhow::Result<u32> {
            let mut u32buf = [0u8; 4];
            r.read_exact(&mut u32buf)?;
            Ok(u32::from_le_bytes(u32buf))
        }

        let header = FileHeader {
            magic: read_u32(&mut file)?,
            _header: read_u32(&mut file)?,
            _version: read_u32(&mut file)?,
            ntoc: read_u32(&mut file)?,
        };
        ensure!(
            header.magic == MAGIC,
            "magic number doesn't match 0x{:x} != expected 0x{:x}",
            header.magic,
            MAGIC
        );

        let mut toc = vec![];
        for _ in 0..header.ntoc {
            toc.push(Toc {
                type_: read_u32(&mut file)?,
                subtype: read_u32(&mut file)?,
                position: read_u32(&mut file)?,
            });
        }

        ensure!(!toc.is_empty(), "no images are present");

        let size = self.size.unwrap_or(24) as isize;
        let mut best = None;
        for item in &toc {
            if item.type_ != IMAGE_TYPE {
                continue;
            }
            let distance = ((item.subtype as isize) - size).abs();
            match best.take() {
                None => {
                    best.replace((item, distance));
                }
                Some((other_item, other_dist)) => {
                    best.replace(if distance < other_dist {
                        (item, distance)
                    } else {
                        (other_item, other_dist)
                    });
                }
            }
        }

        let item = best
            .take()
            .ok_or_else(|| anyhow::anyhow!("no matching images"))?
            .0;

        file.seek(SeekFrom::Start(item.position.into()))?;

        let _chunk_header = read_u32(&mut file)?;
        let chunk_type = read_u32(&mut file)?;
        let chunk_subtype = read_u32(&mut file)?;
        let _chunk_version = read_u32(&mut file)?;

        ensure!(
            chunk_type == item.type_,
            "chunk_type {:x} != item.type_ {:x}",
            chunk_type,
            item.type_
        );
        ensure!(
            chunk_subtype == item.subtype,
            "chunk_subtype {:x} != item.subtype {:x}",
            chunk_subtype,
            item.subtype
        );

        let width = read_u32(&mut file)?;
        let height = read_u32(&mut file)?;
        let xhot = read_u32(&mut file)?;
        let yhot = read_u32(&mut file)?;
        let _delay = read_u32(&mut file)?;

        let num_pixels = (width as usize) * (height as usize);
        ensure!(
            num_pixels < u32::max_value() as usize,
            "cursor image is larger than fits in u32"
        );

        let mut pixels = vec![0u8; num_pixels * 4];
        file.read_exact(&mut pixels)?;

        // The data is all little endian; convert to host order
        for chunk in pixels.chunks_exact_mut(4) {
            let mut data = [0u8; 4];
            data.copy_from_slice(chunk);
            let le = u32::from_le_bytes(data);
            data = le.to_le_bytes();
            chunk.copy_from_slice(&data);
        }

        let image = XcbImage::create_native(
            conn,
            width.try_into()?,
            height.try_into()?,
            xcb::x::ImageFormat::ZPixmap as u32,
            32,
            std::ptr::null_mut(),
            pixels.len() as u32,
            pixels.as_mut_ptr(),
        )?;

        let pixmap = conn.generate_id();
        conn.send_request_no_reply(&xcb::x::CreatePixmap {
            depth: 32,
            pid: pixmap,
            drawable: xcb::x::Drawable::Window(conn.root),
            width: width as u16,
            height: height as u16,
        })
        .context("create_pixmap")?;

        let gc = conn.generate_id();
        conn.send_request_no_reply(&xcb::x::CreateGc {
            cid: gc,
            drawable: xcb::x::Drawable::Pixmap(pixmap),
            value_list: &[],
        })
        .context("CreateGc")?;

        image.put(conn, pixmap.resource_id(), gc.resource_id(), 0, 0, 0);

        conn.send_request_no_reply(&xcb::x::FreeGc { gc })?;

        let pic = conn.generate_id();
        conn.send_request_no_reply(&xcb::render::CreatePicture {
            pid: pic,
            drawable: xcb::x::Drawable::Pixmap(pixmap),
            format: self.pict_format_id.unwrap(),
            value_list: &[],
        })
        .context("create_picture")?;

        conn.send_request_no_reply(&xcb::x::FreePixmap { pixmap })?;

        let cursor_id: Cursor = conn.generate_id();
        conn.send_request_no_reply(&xcb::render::CreateCursor {
            cid: cursor_id,
            source: pic,
            x: xhot.try_into()?,
            y: yhot.try_into()?,
        })
        .context("create_cursor")?;

        conn.send_request_no_reply(&xcb::render::FreePicture { picture: pic })?;

        Ok(cursor_id)
    }
}

// The index.theme file looks something like this:
//
// [Icon Theme]
// Inherits=Adwaita
//
// This function extracts the inherited theme name from it.
fn extract_inherited_theme_name(p: PathBuf) -> Option<String> {
    let data = std::fs::read_to_string(&p).ok()?;
    log::trace!("Parsing {p:?} to determine inheritance");
    for line in data.lines() {
        let fields: Vec<&str> = line.splitn(2, '=').collect();
        if fields.len() == 2 {
            let key = fields[0].trim();
            if key == "Inherits" {
                fn separator(c: char) -> bool {
                    c.is_whitespace() || c == ';' || c == ','
                }

                return Some(
                    fields[1]
                        .trim()
                        .chars()
                        .skip_while(|&c| separator(c))
                        .take_while(|&c| !separator(c))
                        .collect(),
                );
            }
        }
    }
    None
}
