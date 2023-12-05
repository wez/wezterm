use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{CreateEventW, ResetEvent, SetEvent};

pub struct EventHandle(pub HANDLE);
unsafe impl Send for EventHandle {}
unsafe impl Sync for EventHandle {}

impl Drop for EventHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

impl EventHandle {
    pub fn new_manual_reset() -> anyhow::Result<Self> {
        let handle = unsafe { CreateEventW(None, true, false, PCWSTR::null()) }?;
        Ok(Self(handle))
    }

    pub fn set_event(&self) {
        unsafe {
            let _ = SetEvent(self.0);
        }
    }

    pub fn reset_event(&self) {
        unsafe {
            let _ = ResetEvent(self.0);
        }
    }
}
