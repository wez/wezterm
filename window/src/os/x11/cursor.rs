use crate::x11::XConnection;
use crate::MouseCursor;
use std::collections::HashMap;
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
}

impl CursorInfo {
    pub fn new(conn: Weak<XConnection>) -> Self {
        Self {
            cursors: HashMap::new(),
            cursor: None,
            conn,
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
            None => {
                let id_no = match cursor.unwrap_or(MouseCursor::Arrow) {
                    // `/usr/include/X11/cursorfont.h`
                    MouseCursor::Arrow => 132,
                    MouseCursor::Hand => 58,
                    MouseCursor::Text => 152,
                    MouseCursor::SizeUpDown => 116,
                    MouseCursor::SizeLeftRight => 108,
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
        };

        xcb::change_window_attributes(&conn, window_id, &[(xcb::ffi::XCB_CW_CURSOR, cursor_id)]);

        self.cursor = cursor;

        Ok(())
    }
}
