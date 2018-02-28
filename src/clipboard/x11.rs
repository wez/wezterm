//! Interface with the X11 clipboard/selection
//! Check out https://tronche.com/gui/x/icccm/sec-2.html for some deep and complex
//! background on what's happening in here.
use failure::{self, Error};
use mio::{Events, Poll, PollOpt, Ready, Token};
use mio::unix::EventedFd;
use mio_extras::channel::{channel as mio_channel, Receiver as MioReceiver, Sender as MioSender};
use std::os::unix::io::AsRawFd;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use wakeup::{Wakeup, WakeupMsg};
use xcb;
use xcb_util;

use clipboard::{ClipboardImpl, Paste};

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

struct Inner {
    /// if we own the clipboard, here's its string content
    owned: Option<String>,
    receiver: MioReceiver<ClipRequest>,
    sender: Sender<Paste>,
    conn: xcb::Connection,
    window_id: xcb::xproto::Window,
    atom_utf8_string: xcb::Atom,
    atom_xsel_data: xcb::Atom,
    atom_targets: xcb::Atom,
    atom_clipboard: xcb::Atom,
    wakeup: Wakeup,
}

impl Inner {
    fn new(
        receiver: MioReceiver<ClipRequest>,
        sender: Sender<Paste>,
        wakeup: Wakeup,
    ) -> Result<Self, Error> {
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

        Ok(Inner {
            conn,
            owned: None,
            receiver,
            sender,
            window_id,
            atom_utf8_string,
            atom_xsel_data,
            atom_targets,
            atom_clipboard,
            wakeup,
        })
    }

    fn send(&mut self, packet: Paste) -> Result<(), Error> {
        match self.sender.send(packet) {
            Ok(_) => {
                self.wakeup.send(WakeupMsg::Paste)?;
                Ok(())
            }
            Err(err) => {
                self.wakeup.send(WakeupMsg::Paste)?;
                bail!("clipboard: error sending to channel: {:?}", err);
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

    fn selection_clear(&mut self) -> Result<(), Error> {
        // Someone else now owns the selection
        eprintln!("SELECTION_CLEAR received");
        self.owned = None;
        self.send(Paste::Cleared)
    }

    fn selection_notify(&mut self, selection: &xcb::SelectionNotifyEvent) -> Result<(), Error> {
        eprintln!("SELECTION_NOTIFY received");

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
                Ok(prop) => self.send(Paste::All(prop.name().into())),
                Err(err) => {
                    eprintln!("clipboard: err while getting clipboard property: {:?}", err);
                    self.send(Paste::All("".into()))
                }
            }
        } else {
            self.send(Paste::All("".into()))
        }
    }

    fn selection_request(&mut self, request: &xcb::SelectionRequestEvent) -> Result<(), Error> {
        // Someone is asking for our selected text
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
        Ok(())
    }

    fn process_xcb_event(&mut self, event: xcb::GenericEvent) -> Result<(), Error> {
        match event.response_type() & 0x7f {
            xcb::SELECTION_CLEAR => self.selection_clear(),
            xcb::SELECTION_NOTIFY => self.selection_notify(unsafe { xcb::cast_event(&event) }),

            xcb::SELECTION_REQUEST => self.selection_request(unsafe { xcb::cast_event(&event) }),
            xcb::MAPPING_NOTIFY => {
                // Nothing to do here; just don't want to print an error
                // in the case below.
                Ok(())
            }
            _ => {
                eprintln!(
                    "clipboard: got unhandled XCB event type {}",
                    event.response_type() & 0x7f
                );
                // Don't bail here; we just want to log the event and carry on.
                Ok(())
            }
        }
    }

    fn process_queued_xcb(&mut self) -> Result<(), Error> {
        match self.conn.poll_for_event() {
            None => match self.conn.has_error() {
                Ok(_) => (),
                Err(err) => {
                    bail!("clipboard window connection is broken: {:?}", err);
                }
            },
            Some(event) => match self.process_xcb_event(event) {
                Ok(_) => (),
                Err(err) => return Err(err),
            },
        }
        self.conn.flush();

        loop {
            match self.conn.poll_for_queued_event() {
                None => return Ok(()),
                Some(event) => self.process_xcb_event(event)?,
            }
            self.conn.flush();
        }
    }

    fn process_receiver(&mut self) -> Result<(), Error> {
        match self.receiver.try_recv() {
            Err(TryRecvError::Empty) => Ok(()),
            Err(err) => bail!("clipboard: Error while reading channel: {:?}", err),
            Ok(request) => self.process_one_cliprequest(request),
        }
    }

    fn process_one_cliprequest(&mut self, request: ClipRequest) -> Result<(), Error> {
        match request {
            ClipRequest::Terminate => bail!("requested termination of clip thread"),
            ClipRequest::SetClipboard(clip) => {
                self.owned = clip;
                self.update_owner();
            }
            ClipRequest::RequestClipboard => {
                // Find the owner and ask them to send us the buffer
                xcb::convert_selection(
                    &self.conn,
                    self.window_id,
                    xcb::ATOM_PRIMARY,
                    self.atom_utf8_string,
                    self.atom_xsel_data,
                    xcb::CURRENT_TIME,
                );
            }
        }
        Ok(())
    }

    /// Waits for events from either the X server or the main thread of the
    /// application.
    fn clip_thread(&mut self) {
        let poll = Poll::new().expect("mio Poll failed to init");

        poll.register(
            &EventedFd(&self.conn.as_raw_fd()),
            Token(0),
            Ready::readable(),
            PollOpt::level(),
        ).expect("failed to register xcb conn for clipboard with mio");

        poll.register(
            &self.receiver,
            Token(1),
            Ready::readable(),
            PollOpt::level(),
        ).expect("failed to register receiver for clipboard with mio");

        let mut events = Events::with_capacity(2);

        self.sender
            .send(Paste::Running)
            .expect("failed to send Running notice");

        loop {
            match poll.poll(&mut events, None) {
                Ok(_) => for event in &events {
                    if event.token() == Token(0) {
                        match self.process_queued_xcb() {
                            Err(err) => {
                                eprintln!("clipboard: {:?}", err);
                                return;
                            }
                            _ => (),
                        }
                    }
                    if event.token() == Token(1) {
                        match self.process_receiver() {
                            Err(err) => {
                                eprintln!("clipboard: {:?}", err);
                                return;
                            }
                            _ => (),
                        }
                        self.conn.flush();
                    }
                },
                Err(err) => {
                    eprintln!("clipboard: {:?}", err);
                    return;
                }
            }
        }
    }
}

/// A clipboard client allows getting or setting the clipboard.
pub struct Clipboard {
    sender: MioSender<ClipRequest>,
    receiver: Receiver<Paste>,
    /// This isn't really dead; we're keeping it alive until we Drop.
    #[allow(dead_code)]
    clip_thread: JoinHandle<()>,
}

impl ClipboardImpl for Clipboard {
    /// Create a new clipboard instance.  `ping` is
    fn new(wakeup: Wakeup) -> Result<Self, Error> {
        let (sender_clip, receiver_clip) = mio_channel();
        let (sender_paste, receiver_paste) = channel();
        let clip_thread = thread::spawn(move || {
            match Inner::new(receiver_clip, sender_paste, wakeup) {
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
    fn set_clipboard(&self, text: Option<String>) -> Result<(), Error> {
        self.sender.send(ClipRequest::SetClipboard(text))?;
        Ok(())
    }

    /// Blocks until the clipboard contents have been retrieved
    fn get_clipboard(&self) -> Result<String, Error> {
        self.sender.send(ClipRequest::RequestClipboard)?;
        match self.receiver.recv_timeout(Duration::from_secs(10)) {
            Ok(Paste::All(result)) => return Ok(result),
            Ok(Paste::Cleared) => return Ok("".into()),
            other @ _ => bail!("unexpected result while waiting for paste: {:?}", other),
        }
    }

    fn try_get_paste(&self) -> Result<Option<Paste>, Error> {
        match self.receiver.try_recv() {
            Err(TryRecvError::Empty) => Ok(None),
            Err(err) => bail!("{:?}", err),
            Ok(paste) => Ok(Some(paste)),
        }
    }
}

impl Drop for Clipboard {
    fn drop(&mut self) {
        self.sender.send(ClipRequest::Terminate).ok();
    }
}
