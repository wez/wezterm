pub(crate) fn prefer_swrast() -> bool {
    #[cfg(windows)]
    {
        if crate::os::windows::is_running_in_rdp_session() {
            // Using OpenGL in RDP has problematic behavior upon
            // disconnect, so we force the use of software rendering.
            log::trace!("Running in an RDP session, use SWRAST");
            return true;
        }
    }
    config::configuration().front_end == config::FrontEndSelection::Software
}
