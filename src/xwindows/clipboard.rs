//! Interface with the X11 clipboard/selection
use failure::{self, Error};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use xcb;
use xcb_util;

/// A fragment of the clipboard data received from another
/// app during paste.
#[derive(Debug)]
pub enum Paste {
    /// The whole content of the paste is available
    All(String),
    /// Someone else now owns the selection.  You should
    /// clear the selection locally.
    Cleared,
    /// The clipboard window has initialized successfully
    Running,
}

#[derive(Debug)]
enum ClipRequest {
    /// tell the system that we want to set (or clear) the
    /// selection to the supplied parameter.
    SetClipboard(Option<String>),
    /// Ask the system to send us the clipboard contents
    RequestClipboard,
    /// Ask the clipboard manager to shutdown
    Terminate,
}

struct ClipboardImpl<F: Fn() + Send + 'static> {
    /// if we own the clipboard, here's its string content
    owned: Option<String>,
    receiver: Receiver<ClipRequest>,
    sender: Sender<Paste>,
    conn: xcb::Connection,
    window_id: xcb::xproto::Window,
    atom_utf8_string: xcb::Atom,
    atom_xsel_data: xcb::Atom,
    atom_targets: xcb::Atom,
    atom_clipboard: xcb::Atom,
    ping: F,
}

impl<F: Fn() + Send + 'static> ClipboardImpl<F> {
    fn new(receiver: Receiver<ClipRequest>, sender: Sender<Paste>, ping: F) -> Result<Self, Error> {
        let (conn, screen) = xcb::Connection::connect(None)?;

        let atom_utf8_string = xcb::intern_atom(&conn, false, "UTF8_STRING")
            .get_reply()?
            .atom();
        let atom_xsel_data = xcb::intern_atom(&conn, false, "XSEL_DATA")
            .get_reply()?
            .atom();
        let atom_targets = xcb::intern_atom(&conn, false, "TARGETS")
            .get_reply()?
            .atom();
        let atom_clipboard = xcb::intern_atom(&conn, false, "CLIPBOARD")
            .get_reply()?
            .atom();

        let window_id = conn.generate_id();
        {
            let setup = conn.get_setup();
            let screen = setup
                .roots()
                .nth(screen as usize)
                .ok_or(failure::err_msg("no screen?"))?;

            xcb::create_window_checked(
                &conn,
                xcb::COPY_FROM_PARENT as u8,
                window_id,
                screen.root(),
                // x, y
                0,
                0,
                // width, height
                1,
                1,
                // border width
                0,
                xcb::WINDOW_CLASS_INPUT_ONLY as u16,
                screen.root_visual(),
                &[(xcb::CW_EVENT_MASK, 0)],
            ).request_check()?;
        }

        Ok(ClipboardImpl {
            conn,
            owned: None,
            receiver,
            sender,
            window_id,
            atom_utf8_string,
            atom_xsel_data,
            atom_targets,
            atom_clipboard,
            ping,
        })
    }

    fn send(&self, packet: Paste) -> bool {
        match self.sender.send(packet) {
            Ok(_) => {
                (self.ping)();
                true
            }
            Err(err) => {
                eprintln!("clipboard: error sending to channel: {:?}", err);
                (self.ping)();
                false
            }
        }
    }

    /// Inform X that we either own the selection or that we no longer own the
    /// selection.
    fn update_owner(&self) {
        let owner = if self.owned.is_some() {
            self.window_id
        } else {
            xcb::NONE
        };
        xcb::set_selection_owner(&self.conn, owner, xcb::ATOM_PRIMARY, xcb::CURRENT_TIME);
        // Also set the CLIPBOARD atom, not just the PRIMARY selection.
        // TODO: make xterm clipboard selection configurable
        xcb::set_selection_owner(&self.conn, owner, self.atom_clipboard, xcb::CURRENT_TIME);
    }

    /// Waits for events from either the X server or the main thread of the
    /// application.
    fn clip_thread(&mut self) {
        self.sender
            .send(Paste::Running)
            .expect("failed to send Running notice");

        loop {
            match self.conn.poll_for_event() {
                None => match self.conn.has_error() {
                    Ok(_) => {}
                    Err(err) => {
                        eprintln!("clipboard window connection is broken: {:?}", err);
                        return;
                    }
                },
                Some(event) => match event.response_type() & 0x7f {
                    xcb::SELECTION_CLEAR => {
                        // Someone else now owns the selection
                        eprintln!("SELECTION_CLEAR received");
                        self.owned = None;
                        if !self.send(Paste::Cleared) {
                            return;
                        }
                    }
                    xcb::SELECTION_NOTIFY => {
                        eprintln!("SELECTION_NOTIFY received");
                        let selection: &xcb::SelectionNotifyEvent =
                            unsafe { xcb::cast_event(&event) };

                        if (selection.selection() == xcb::ATOM_PRIMARY
                            || selection.selection() == self.atom_clipboard)
                            && selection.property() != xcb::NONE
                        {
                            match xcb_util::icccm::get_text_property(
                                &self.conn,
                                selection.requestor(),
                                selection.property(),
                            ).get_reply()
                            {
                                Ok(prop) => {
                                    if !self.send(Paste::All(prop.name().into())) {
                                        return;
                                    }
                                }
                                Err(err) => {
                                    eprintln!(
                                        "clipboard: err while getting clipboard property: {:?}",
                                        err
                                    );
                                    if !self.send(Paste::All("".into())) {
                                        return;
                                    }
                                }
                            }
                        } else if !self.send(Paste::All("".into())) {
                            return;
                        }
                    }
                    xcb::SELECTION_REQUEST => {
                        // Someone is asking for our selected text

                        let request: &xcb::SelectionRequestEvent =
                            unsafe { xcb::cast_event(&event) };
                        debug!(
                            "SEL: time={} owner={} requestor={} selection={} target={} property={}",
                            request.time(),
                            request.owner(),
                            request.requestor(),
                            request.selection(),
                            request.target(),
                            request.property()
                        );
                        debug!(
                            "XSEL={}, UTF8={} PRIMARY={} clip={}",
                            self.atom_xsel_data,
                            self.atom_utf8_string,
                            xcb::ATOM_PRIMARY,
                            self.atom_clipboard,
                        );

                        // I'd like to use `match` here, but the atom values are not
                        // known at compile time so we have to `if` like a caveman :-p
                        let selprop = if request.target() == self.atom_targets {
                            // They want to know which targets we support
                            debug!("responding with targets list");
                            let atoms: [u32; 1] = [self.atom_utf8_string];
                            xcb::xproto::change_property(
                                &self.conn,
                                xcb::xproto::PROP_MODE_REPLACE as u8,
                                request.requestor(),
                                request.property(),
                                xcb::xproto::ATOM_ATOM,
                                32, /* 32-bit atom value */
                                &atoms,
                            );

                            // let the requestor know that we set their property
                            request.property()
                        } else if request.target() == self.atom_utf8_string
                            || request.target() == xcb::xproto::ATOM_STRING
                        {
                            // We'll accept requests for UTF-8 or STRING data.
                            // We don't and won't do any conversion from UTF-8 to
                            // whatever STRING represents; let's just assume that
                            // the other end is going to handle it correctly.
                            if let &Some(ref text) = &self.owned {
                                debug!("going to respond with clip text because we own it");
                                xcb::xproto::change_property(
                                    &self.conn,
                                    xcb::xproto::PROP_MODE_REPLACE as u8,
                                    request.requestor(),
                                    request.property(),
                                    request.target(),
                                    8, /* 8-bit string data */
                                    text.as_bytes(),
                                );
                                // let the requestor know that we set their property
                                request.property()
                            } else {
                                debug!("we don't own the clipboard");
                                // We have no clipboard so there is nothing to report
                                xcb::NONE
                            }
                        } else {
                            debug!("don't know what to do");
                            // We didn't support their request, so there is nothing
                            // we can report back to them.
                            xcb::NONE
                        };
                        debug!("responding with selprop={}", selprop);

                        xcb::xproto::send_event(
                            &self.conn,
                            true,
                            request.requestor(),
                            0,
                            &xcb::xproto::SelectionNotifyEvent::new(
                                request.time(),
                                request.requestor(),
                                request.selection(),
                                request.target(),
                                selprop, // the disposition from the operation above
                            ),
                        );
                        self.conn.flush();
                    }
                    _ => {
                        eprintln!(
                            "clipboard: got unhandled XCB event type {}",
                            event.response_type() & 0x7f
                        );
                    }
                },
            }

            match self.receiver.recv_timeout(Duration::from_millis(100)) {
                Err(RecvTimeoutError::Timeout) => continue,
                Err(err) => {
                    eprintln!("clipboard: Error while reading channel: {:?}", err);
                    return;
                }
                Ok(ClipRequest::Terminate) => break,
                Ok(ClipRequest::SetClipboard(clip)) => {
                    self.owned = clip;
                    self.update_owner();
                    self.conn.flush();
                }
                Ok(ClipRequest::RequestClipboard) => {
                    // Find the owner and ask them to send us the buffer
                    xcb::convert_selection(
                        &self.conn,
                        self.window_id,
                        xcb::ATOM_PRIMARY,
                        self.atom_utf8_string,
                        self.atom_xsel_data,
                        xcb::CURRENT_TIME,
                    );
                    self.conn.flush();
                }
            }
        }
    }
}

/// A clipboard client allows getting or setting the clipboard.
pub struct Clipboard {
    sender: Sender<ClipRequest>,
    receiver: Receiver<Paste>,
    clip_thread: JoinHandle<()>,
}

impl Clipboard {
    /// Create a new clipboard instance.  `ping` is
    pub fn new<F>(ping: F) -> Result<Self, Error>
    where
        F: Fn() + Send + 'static,
    {
        let (sender_clip, receiver_clip) = channel();
        let (sender_paste, receiver_paste) = channel();
        let clip_thread = thread::spawn(move || {
            match ClipboardImpl::new(receiver_clip, sender_paste, ping) {
                Ok(mut clip) => clip.clip_thread(),
                Err(err) => eprintln!("failed to init clipboard window: {:?}", err),
            }
        });

        // Make sure that it started up ok
        match receiver_paste.recv_timeout(Duration::from_secs(10)) {
            Ok(Paste::Running) => {}
            other @ _ => bail!("failed to init clipboard window: {:?}", other),
        };

        Ok(Self {
            sender: sender_clip,
            receiver: receiver_paste,
            clip_thread,
        })
    }

    /// Tell X that we own the selection and its contents are `text`
    pub fn set_clipboard(&self, text: Option<String>) -> Result<(), Error> {
        self.sender.send(ClipRequest::SetClipboard(text))?;
        Ok(())
    }

    /// Ask the selection owner for the clipboard contents.
    /// The contents will be delivered asynchronously via
    /// the receiver.
    pub fn request_clipboard(&self) -> Result<(), Error> {
        self.sender.send(ClipRequest::RequestClipboard)?;
        Ok(())
    }

    pub fn receiver(&self) -> &Receiver<Paste> {
        &self.receiver
    }

    /// Blocks until the clipboard contents have been retrieved
    pub fn get_clipboard(&self) -> Result<String, Error> {
        self.request_clipboard()?;
        match self.receiver().recv_timeout(Duration::from_secs(10)) {
            Ok(Paste::All(result)) => return Ok(result),
            Ok(Paste::Cleared) => return Ok("".into()),
            other @ _ => bail!("unexpected result while waiting for paste: {:?}", other),
        }
    }
}

impl Drop for Clipboard {
    fn drop(&mut self) {
        self.sender.send(ClipRequest::Terminate).ok();
    }
}
