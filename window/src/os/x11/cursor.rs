use crate::x11::XConnection;
use crate::MouseCursor;
use anyhow::{ensure, Context};
use std::collections::HashMap;
use std::convert::TryInto;
use std::ffi::OsString;
use std::io::prelude::*;
use std::io::SeekFrom;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use xcb::ffi::xcb_cursor_t;

pub struct XcbCursor {
    pub id: xcb_cursor_t,
    pub conn: Weak<XConnection>,
}

impl Drop for XcbCursor {
    fn drop(&mut self) {
        if let Some(conn) = self.conn.upgrade() {
            xcb::free_cursor(&conn.conn, self.id);
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
    let path = std::env::var_os("XCURSOR_PATH").unwrap_or_else(|| {
        OsString::from("~/.icons:/usr/share/icons:/usr/share/pixmaps:/usr/X11R6/lib/X11/icons")
    });

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

fn cursor_size(map: &HashMap<String, String>) -> u32 {
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
    pub fn new(conn: &Rc<XConnection>) -> Self {
        let mut size = None;
        let mut theme = None;
        let mut pict_format_id = None;
        // If we know the theme to use, then we need the render extension
        // if we are to be able to load the cursor
        let has_render = unsafe {
            conn.get_extension_data(&mut xcb::ffi::render::xcb_render_id)
                .map_or(false, |ext| ext.present())
        };
        if has_render {
            if let Ok(vers) = xcb::render::query_version(
                conn.conn(),
                xcb::ffi::render::XCB_RENDER_MAJOR_VERSION,
                xcb::ffi::render::XCB_RENDER_MINOR_VERSION,
            )
            .get_reply()
            {
                // 0.5 and later have the required support
                if (vers.major_version(), vers.minor_version()) >= (0, 5) {
                    size.replace(cursor_size(&conn.xrm));
                    theme = conn.xrm.get("Xcursor.theme").cloned();

                    // Locate the Pictformat corresponding to ARGB32
                    if let Ok(formats) = xcb::render::query_pict_formats(conn.conn()).get_reply() {
                        for fmt in formats.formats() {
                            if fmt.depth() == 32 {
                                let direct = fmt.direct();
                                if direct.alpha_shift() == 24
                                    && direct.red_shift() == 16
                                    && direct.green_shift() == 8
                                    && direct.blue_shift() == 0
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
        window_id: xcb::xproto::Window,
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

        xcb::change_window_attributes(&conn, window_id, &[(xcb::ffi::XCB_CW_CURSOR, cursor_id)]);

        self.cursor = cursor;

        Ok(())
    }

    fn load_themed(&mut self, conn: &Rc<XConnection>, cursor: Option<MouseCursor>) -> Option<u32> {
        let theme = self.theme.as_ref()?;
        if self.pict_format_id.is_none() {
            return None;
        }

        let name = match cursor.unwrap_or(MouseCursor::Arrow) {
            MouseCursor::Arrow => "top_left_arrow",
            MouseCursor::Hand => "hand2",
            MouseCursor::Text => "xterm",
            MouseCursor::SizeUpDown => "sb_v_double_arrow",
            MouseCursor::SizeLeftRight => "sb_h_double_arrow",
        };

        for dir in &self.icon_path {
            let candidate = dir.join(theme).join("cursors").join(name);
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

                        return Some(cursor_id);
                    }
                    Err(err) => log::error!("{:#}", err),
                }
            }
        }
        None
    }

    fn load_basic(&mut self, conn: &Rc<XConnection>, cursor: Option<MouseCursor>) -> u32 {
        let id_no = match cursor.unwrap_or(MouseCursor::Arrow) {
            // `/usr/include/X11/cursorfont.h`
            // <https://docs.rs/xcb-util/0.3.0/src/xcb_util/cursor.rs.html>
            MouseCursor::Arrow => xcb_util::cursor::TOP_LEFT_ARROW,
            MouseCursor::Hand => xcb_util::cursor::HAND1,
            MouseCursor::Text => xcb_util::cursor::XTERM,
            MouseCursor::SizeUpDown => xcb_util::cursor::SB_V_DOUBLE_ARROW,
            MouseCursor::SizeLeftRight => xcb_util::cursor::SB_H_DOUBLE_ARROW,
        };

        let cursor_id: xcb::ffi::xcb_cursor_t = conn.generate_id();
        xcb::create_glyph_cursor(
            &conn,
            cursor_id,
            conn.cursor_font_id,
            conn.cursor_font_id,
            id_no,
            id_no + 1,
            0xffff,
            0xffff,
            0xffff,
            0,
            0,
            0,
        );

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
    ) -> anyhow::Result<u32> {
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
            header: u32,
            version: u32,
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
            header: read_u32(&mut file)?,
            version: read_u32(&mut file)?,
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

        let image = unsafe {
            xcb_util::ffi::image::xcb_image_create_native(
                conn.conn().get_raw_conn(),
                width.try_into()?,
                height.try_into()?,
                xcb::xproto::IMAGE_FORMAT_Z_PIXMAP,
                32,
                std::ptr::null_mut(),
                pixels.len() as u32,
                pixels.as_mut_ptr(),
            )
        };
        ensure!(!image.is_null(), "failed to create native image");

        let pixmap = conn.generate_id();
        xcb::xproto::create_pixmap_checked(
            conn,
            32,
            pixmap,
            conn.root,
            width as u16,
            height as u16,
        )
        .request_check()
        .context("create_pixmap")?;

        let gc = conn.generate_id();
        xcb::create_gc(conn.conn(), gc, pixmap, &[]);

        unsafe {
            xcb_util::ffi::image::xcb_image_put(
                conn.conn().get_raw_conn(),
                pixmap,
                gc,
                image,
                0,
                0,
                0,
            )
        };

        xcb::free_gc(conn.conn(), gc);

        let pic = conn.generate_id();
        xcb::render::create_picture_checked(
            conn.conn(),
            pic,
            pixmap,
            self.pict_format_id.unwrap(),
            &[],
        )
        .request_check()
        .context("create_picture")?;

        xcb::xproto::free_pixmap(conn.conn(), pixmap);

        let cursor_id: xcb::ffi::xcb_cursor_t = conn.generate_id();
        xcb::render::create_cursor_checked(
            conn.conn(),
            cursor_id,
            pic,
            xhot.try_into()?,
            yhot.try_into()?,
        )
        .request_check()
        .context("create_cursor")?;

        xcb::render::free_picture(conn.conn(), pic);
        unsafe {
            xcb_util::ffi::image::xcb_image_destroy(image);
        }

        Ok(cursor_id)
    }
}
