use crate::bitmaps::BitmapImage;
use failure::Fallible;
use std::io::Error as IoError;
use winapi::shared::windef::*;
use winapi::um::wingdi::*;

pub struct GdiBitmap {
    hdc: HDC,
    hbitmap: HBITMAP,
    data: *mut u8,
    width: usize,
    height: usize,
}

impl BitmapImage for GdiBitmap {
    unsafe fn pixel_data(&self) -> *const u8 {
        self.data
    }

    unsafe fn pixel_data_mut(&mut self) -> *mut u8 {
        self.data
    }

    fn image_dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

impl Drop for GdiBitmap {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.hbitmap as _);
        }
        unsafe {
            DeleteObject(self.hdc as _);
        }
    }
}

impl GdiBitmap {
    pub fn hdc(&self) -> HDC {
        self.hdc
    }

    pub fn hbitmap(&self) -> HBITMAP {
        self.hbitmap
    }

    pub fn new_compatible(width: usize, height: usize, hdc: HDC) -> Fallible<Self> {
        let hdc = unsafe { CreateCompatibleDC(hdc) };
        if hdc.is_null() {
            let err = IoError::last_os_error();
            failure::bail!("CreateCompatibleDC: {}", err);
        }

        let mut data = std::ptr::null_mut();
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biPlanes: 1,
                biBitCount: 32,
                biWidth: width as i32,
                // Windows bitmaps are upside-down vs. the rest of the world, so
                // we need to supply a negative height here:
                // https://stackoverflow.com/a/9023702/149111
                biHeight: -(height as i32),
                biClrImportant: 0,
                biClrUsed: 0,
                biCompression: 0,
                biSizeImage: width as u32 * height as u32 * 4,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
            },
            bmiColors: [RGBQUAD {
                rgbBlue: 0,
                rgbRed: 0,
                rgbGreen: 0,
                rgbReserved: 0,
            }],
        };
        let hbitmap = unsafe {
            CreateDIBSection(
                hdc,
                &bmi,
                DIB_RGB_COLORS,
                &mut data,
                std::ptr::null_mut(),
                0,
            )
        };

        if hbitmap.is_null() {
            let err = IoError::last_os_error();
            failure::bail!("CreateDIBSection: {}", err);
        }

        unsafe {
            SelectObject(hdc, hbitmap as _);
        }

        Ok(Self {
            hdc,
            hbitmap,
            data: data as *mut u8,
            width,
            height,
        })
    }
}
