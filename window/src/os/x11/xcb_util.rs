#![allow(non_camel_case_types)]
use libc::c_void;
use xcb::ffi::*;

pub type xcb_image_format_t = u32;
pub type xcb_drawable_t = u32;
pub type xcb_gcontext_t = u32;
pub type xcb_void_cookie_t = u64;
pub type xcb_image_t = ();

pub struct XcbImage(*mut xcb_image_t);

impl Drop for XcbImage {
    fn drop(&mut self) {
        unsafe {
            xcb_image_destroy(self.0);
        }
    }
}

impl XcbImage {
    pub fn create_native(
        c: &xcb::Connection,
        width: u16,
        height: u16,
        format: xcb_image_format_t,
        depth: u8,
        base: *mut c_void,
        bytes: u32,
        data: *mut u8,
    ) -> anyhow::Result<Self> {
        let image = unsafe {
            xcb_image_create_native(
                c.get_raw_conn(),
                width,
                height,
                format,
                depth,
                base,
                bytes,
                data,
            )
        };
        if image.is_null() {
            anyhow::bail!("failed to create native image");
        } else {
            Ok(Self(image))
        }
    }

    pub fn put(
        &self,
        conn: &xcb::Connection,
        draw: xcb_drawable_t,
        gc: xcb_gcontext_t,
        x: i16,
        y: i16,
        left_pad: u8,
    ) -> xcb_void_cookie_t {
        unsafe { xcb_image_put(conn.get_raw_conn(), draw, gc, self.0, x, y, left_pad) }
    }
}

#[link(name = "xcb-image")]
extern "C" {
    pub fn xcb_image_create_native(
        c: *mut xcb_connection_t,
        width: u16,
        height: u16,
        format: xcb_image_format_t,
        depth: u8,
        base: *mut c_void,
        bytes: u32,
        data: *mut u8,
    ) -> *mut xcb_image_t;
    pub fn xcb_image_destroy(image: *mut xcb_image_t);
    pub fn xcb_image_put(
        conn: *mut xcb_connection_t,
        draw: xcb_drawable_t,
        gc: xcb_gcontext_t,
        image: *const xcb_image_t,
        x: i16,
        y: i16,
        left_pad: u8,
    ) -> xcb_void_cookie_t;
}

pub const XCB_ICCCM_SIZE_HINT_US_SIZE: u32 = 1 << 1;
pub const XCB_ICCCM_SIZE_HINT_P_POSITION: u32 = 1 << 2;
pub const XCB_ICCCM_SIZE_HINT_P_SIZE: u32 = 1 << 3;
pub const XCB_ICCCM_SIZE_HINT_P_MIN_SIZE: u32 = 1 << 4;
pub const XCB_ICCCM_SIZE_HINT_P_MAX_SIZE: u32 = 1 << 5;
pub const XCB_ICCCM_SIZE_HINT_P_RESIZE_INC: u32 = 1 << 6;
pub const XCB_ICCCM_SIZE_HINT_P_ASPECT: u32 = 1 << 7;
pub const XCB_ICCCM_SIZE_HINT_BASE_SIZE: u32 = 1 << 8;
pub const XCB_ICCCM_SIZE_HINT_P_WIN_GRAVITY: u32 = 1 << 9;

#[repr(C)]
pub struct xcb_size_hints_t {
    pub flags: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub min_width: i32,
    pub min_height: i32,
    pub max_width: i32,
    pub max_height: i32,
    pub width_inc: i32,
    pub height_inc: i32,
    pub min_aspect_num: i32,
    pub min_aspect_den: i32,
    pub max_aspect_num: i32,
    pub max_aspect_den: i32,
    pub base_width: i32,
    pub base_height: i32,
    pub win_gravity: u32,
}

pub const MOVE_RESIZE_WINDOW_X: u32 = 1 << 8;
pub const MOVE_RESIZE_WINDOW_Y: u32 = 1 << 9;
pub const MOVE_RESIZE_WINDOW_WIDTH: u32 = 1 << 10;
pub const MOVE_RESIZE_WINDOW_HEIGHT: u32 = 1 << 11;

pub const MOVE_RESIZE_SIZE_TOPLEFT: u32 = 0;
pub const MOVE_RESIZE_SIZE_TOP: u32 = 1;
pub const MOVE_RESIZE_SIZE_TOPRIGHT: u32 = 2;
pub const MOVE_RESIZE_SIZE_RIGHT: u32 = 3;
pub const MOVE_RESIZE_SIZE_BOTTOMRIGHT: u32 = 4;
pub const MOVE_RESIZE_SIZE_BOTTOM: u32 = 5;
pub const MOVE_RESIZE_SIZE_BOTTOMLEFT: u32 = 6;
pub const MOVE_RESIZE_SIZE_LEFT: u32 = 7;
pub const MOVE_RESIZE_MOVE: u32 = 8;
pub const MOVE_RESIZE_SIZE_KEYBOARD: u32 = 9;
pub const MOVE_RESIZE_MOVE_KEYBOARD: u32 = 10;
pub const MOVE_RESIZE_CANCEL: u32 = 11;
