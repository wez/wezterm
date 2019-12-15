use super::*;
use crate::bitmaps::*;
use anyhow::bail;
use std::rc::Rc;

/// The X protocol allows referencing a number of drawable
/// objects.  This trait marks those objects here in code.
pub trait Drawable {
    fn as_drawable(&self) -> xcb::xproto::Drawable;
}

impl Drawable for xcb::xproto::Window {
    fn as_drawable(&self) -> xcb::xproto::Drawable {
        *self
    }
}

pub struct Context {
    gc_id: xcb::xproto::Gcontext,
    conn: Rc<XConnection>,
    drawable: xcb::xproto::Drawable,
}

impl Context {
    pub fn new(conn: &Rc<XConnection>, d: &dyn Drawable) -> Context {
        let gc_id = conn.conn().generate_id();
        let drawable = d.as_drawable();
        xcb::create_gc(conn.conn(), gc_id, drawable, &[]);
        Context {
            gc_id,
            conn: Rc::clone(conn),
            drawable,
        }
    }

    /// Copy an area from one drawable to another using the settings
    /// defined in this context.
    #[allow(clippy::too_many_arguments)]
    pub fn copy_area(
        &self,
        src: &dyn Drawable,
        src_x: i16,
        src_y: i16,
        dest: &dyn Drawable,
        dest_x: i16,
        dest_y: i16,
        width: u16,
        height: u16,
    ) -> xcb::VoidCookie {
        xcb::copy_area(
            self.conn.conn(),
            src.as_drawable(),
            dest.as_drawable(),
            self.gc_id,
            src_x,
            src_y,
            dest_x,
            dest_y,
            width,
            height,
        )
    }

    /// Send image bytes and render them into the drawable that was used to
    /// create this context.
    pub fn put_image(&self, dest_x: i16, dest_y: i16, im: &dyn BitmapImage) -> xcb::VoidCookie {
        let (width, height) = im.image_dimensions();
        let pixel_slice =
            unsafe { std::slice::from_raw_parts(im.pixel_data(), width * height * 4) };
        xcb::put_image(
            self.conn.conn(),
            xcb::xproto::IMAGE_FORMAT_Z_PIXMAP as u8,
            self.drawable,
            self.gc_id,
            width as u16,
            height as u16,
            dest_x,
            dest_y,
            0,
            24,
            pixel_slice,
        )
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        xcb::free_gc(self.conn.conn(), self.gc_id);
    }
}

/// Holder for a shared memory segment id.
/// We hold on to the id only until the server has attached
/// (or failed to attach) to the segment.
/// The id is removed on Drop.
struct ShmId {
    id: libc::c_int,
}

/// Holder for a shared memory mapping.
/// The mapping is removed on Drop.
struct ShmData {
    /// the base address of the mapping
    data: *mut u8,
}

impl ShmId {
    /// Create a new private shared memory segment of the specified size
    fn new(size: usize) -> anyhow::Result<ShmId> {
        let id = unsafe { libc::shmget(libc::IPC_PRIVATE, size, libc::IPC_CREAT | 0o600) };
        if id == -1 {
            bail!(
                "shmget failed for {} bytes: {:?}",
                size,
                std::io::Error::last_os_error()
            );
        }
        Ok(ShmId { id })
    }

    /// Attach the segment to our address space
    fn attach(&self) -> anyhow::Result<ShmData> {
        let data = unsafe { libc::shmat(self.id, std::ptr::null(), 0) };
        if data as usize == !0 {
            bail!(
                "shmat failed: {:?} {}",
                data,
                std::io::Error::last_os_error()
            );
        }
        Ok(ShmData {
            data: data as *mut u8,
        })
    }
}

impl Drop for ShmId {
    fn drop(&mut self) {
        unsafe {
            libc::shmctl(self.id, libc::IPC_RMID, std::ptr::null_mut());
        }
    }
}

impl Drop for ShmData {
    fn drop(&mut self) {
        unsafe {
            libc::shmdt(self.data as *const _);
        }
    }
}

/// An image implementation backed by shared memory.
/// This also has an associated pixmap on the server side,
/// so we implement both BitmapImage and Drawable.
pub struct ShmImage {
    data: ShmData,
    seg_id: xcb::shm::Seg,
    draw_id: u32,
    conn: Rc<XConnection>,
    width: usize,
    height: usize,
}

impl ShmImage {
    pub fn new(
        conn: &Rc<XConnection>,
        drawable: xcb::xproto::Drawable,
        width: usize,
        height: usize,
    ) -> anyhow::Result<ShmImage> {
        if !conn.shm_available {
            bail!("SHM not available");
        }

        // Allocate and attach memory of the desired size
        let id = ShmId::new(width * height * 4)?;
        let data = id.attach()?;

        // Tell the server to attach to it
        let seg_id = conn.generate_id();
        xcb::shm::attach_checked(conn, seg_id, id.id as u32, false).request_check()?;

        // Now create a pixmap that references it
        let draw_id = conn.generate_id();
        xcb::shm::create_pixmap_checked(
            conn,
            draw_id,
            drawable,
            width as u16,
            height as u16,
            24,
            seg_id,
            0,
        )
        .request_check()?;

        Ok(ShmImage {
            data,
            seg_id,
            draw_id,
            conn: Rc::clone(conn),
            width,
            height,
        })
    }
}

impl BitmapImage for ShmImage {
    unsafe fn pixel_data(&self) -> *const u8 {
        self.data.data as *const u8
    }

    unsafe fn pixel_data_mut(&mut self) -> *mut u8 {
        self.data.data
    }

    fn image_dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

impl Drop for ShmImage {
    fn drop(&mut self) {
        xcb::free_pixmap(self.conn.conn(), self.draw_id);
        xcb::shm::detach(self.conn.conn(), self.seg_id);
    }
}

impl Drawable for ShmImage {
    fn as_drawable(&self) -> xcb::xproto::Drawable {
        self.draw_id
    }
}

pub enum BufferImage {
    Image(Image),
    Shared(ShmImage),
}

impl BitmapImage for BufferImage {
    fn image_dimensions(&self) -> (usize, usize) {
        match self {
            BufferImage::Image(im) => im.image_dimensions(),
            BufferImage::Shared(im) => im.image_dimensions(),
        }
    }

    unsafe fn pixel_data(&self) -> *const u8 {
        match self {
            BufferImage::Image(im) => im.pixel_data(),
            BufferImage::Shared(im) => im.pixel_data(),
        }
    }

    unsafe fn pixel_data_mut(&mut self) -> *mut u8 {
        match self {
            BufferImage::Image(im) => im.pixel_data_mut(),
            BufferImage::Shared(im) => im.pixel_data_mut(),
        }
    }
}

impl BufferImage {
    pub fn new(
        conn: &Rc<XConnection>,
        drawable: xcb::xproto::Drawable,
        width: usize,
        height: usize,
    ) -> BufferImage {
        match ShmImage::new(conn, drawable, width, height) {
            Ok(shm) => BufferImage::Shared(shm),
            Err(_) => BufferImage::Image(Image::new(width, height)),
        }
    }
}
