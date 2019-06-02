use libc::pollfd;
use std::time::Duration;

pub fn poll(pfd: &mut [pollfd], duration: Option<Duration>) -> Result<usize, std::io::Error> {
    let poll_result = unsafe {
        libc::poll(
            pfd.as_mut_ptr(),
            pfd.len() as _,
            duration
                .map(|wait| wait.as_millis() as libc::c_int)
                .unwrap_or(-1),
        )
    };
    if poll_result < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(poll_result as usize)
    }
}
