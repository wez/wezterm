use crate::config::configuration;
use crate::mux::domain::DomainId;
use crate::mux::renderable::{Renderable, RenderableDimensions};
use crate::mux::tab::{alloc_tab_id, Tab, TabId};
use crate::ratelim::RateLimiter;
use crate::server::codec::*;
use crate::server::domain::ClientInner;
use crate::server::tab::mousestate::MouseState;
use crate::server::tab::renderable::{RenderableInner, RenderableState};
use anyhow::bail;
use filedescriptor::Pipe;
use log::info;
use portable_pty::PtySize;
use std::cell::RefCell;
use std::cell::RefMut;
use std::rc::Rc;
use std::sync::Arc;
use term::color::ColorPalette;
use term::{Clipboard, KeyCode, KeyModifiers, MouseEvent, TerminalHost};
use termwiz::input::KeyEvent;
use url::Url;

pub struct ClientTab {
    client: Arc<ClientInner>,
    local_tab_id: TabId,
    remote_tab_id: TabId,
    pub renderable: RefCell<RenderableState>,
    writer: RefCell<TabWriter>,
    reader: Pipe,
    mouse: Rc<RefCell<MouseState>>,
    clipboard: RefCell<Option<Arc<dyn Clipboard>>>,
    mouse_grabbed: RefCell<bool>,
}

impl ClientTab {
    pub fn new(
        client: &Arc<ClientInner>,
        remote_tab_id: TabId,
        size: PtySize,
        title: &str,
    ) -> Self {
        let local_tab_id = alloc_tab_id();
        let writer = TabWriter {
            client: Arc::clone(client),
            remote_tab_id,
        };

        let mouse = Rc::new(RefCell::new(MouseState::new(
            remote_tab_id,
            client.client.clone(),
        )));

        let fetch_limiter =
            RateLimiter::new(|config| config.ratelimit_mux_line_prefetches_per_second);

        let render = RenderableState {
            inner: RefCell::new(RenderableInner::new(
                client,
                remote_tab_id,
                local_tab_id,
                RenderableDimensions {
                    cols: size.cols as _,
                    viewport_rows: size.rows as _,
                    scrollback_rows: size.rows as _,
                    physical_top: 0,
                    scrollback_top: 0,
                },
                title,
                fetch_limiter,
            )),
        };

        let reader = Pipe::new().expect("Pipe::new failed");

        Self {
            client: Arc::clone(client),
            mouse,
            remote_tab_id,
            local_tab_id,
            renderable: RefCell::new(render),
            writer: RefCell::new(writer),
            reader,
            clipboard: RefCell::new(None),
            mouse_grabbed: RefCell::new(false),
        }
    }

    pub fn process_unilateral(&self, pdu: Pdu) -> anyhow::Result<()> {
        match pdu {
            Pdu::GetTabRenderChangesResponse(delta) => {
                *self.mouse_grabbed.borrow_mut() = delta.mouse_grabbed;
                self.renderable
                    .borrow()
                    .inner
                    .borrow_mut()
                    .apply_changes_to_surface(delta);
            }
            Pdu::SetClipboard(SetClipboard { clipboard, .. }) => {
                match self.clipboard.borrow().as_ref() {
                    Some(clip) => {
                        clip.set_contents(clipboard)?;
                    }
                    None => {
                        log::error!("ClientTab: Ignoring SetClipboard request {:?}", clipboard);
                    }
                }
            }
            _ => bail!("unhandled unilateral pdu: {:?}", pdu),
        };
        Ok(())
    }

    pub fn remote_tab_id(&self) -> TabId {
        self.remote_tab_id
    }
}

impl Tab for ClientTab {
    fn tab_id(&self) -> TabId {
        self.local_tab_id
    }
    fn renderer(&self) -> RefMut<dyn Renderable> {
        self.renderable.borrow_mut()
    }

    fn set_clipboard(&self, clipboard: &Arc<dyn Clipboard>) {
        self.clipboard.borrow_mut().replace(Arc::clone(clipboard));
    }

    fn get_title(&self) -> String {
        let renderable = self.renderable.borrow();
        let inner = renderable.inner.borrow();
        inner.title.clone()
    }

    fn send_paste(&self, text: &str) -> anyhow::Result<()> {
        let client = Arc::clone(&self.client);
        let remote_tab_id = self.remote_tab_id;
        let data = text.to_owned();
        promise::spawn::spawn(async move {
            client
                .client
                .send_paste(SendPaste {
                    tab_id: remote_tab_id,
                    data,
                })
                .await
        });
        self.renderable
            .borrow()
            .inner
            .borrow_mut()
            .update_last_send();
        Ok(())
    }

    fn reader(&self) -> anyhow::Result<Box<dyn std::io::Read + Send>> {
        info!("made reader for ClientTab");
        Ok(Box::new(self.reader.read.try_clone()?))
    }

    fn writer(&self) -> RefMut<dyn std::io::Write> {
        self.writer.borrow_mut()
    }

    fn resize(&self, size: PtySize) -> anyhow::Result<()> {
        let render = self.renderable.borrow();
        let mut inner = render.inner.borrow_mut();

        let cols = size.cols as usize;
        let rows = size.rows as usize;

        if inner.dimensions.cols != cols || inner.dimensions.viewport_rows != rows {
            inner.dimensions.cols = cols;
            inner.dimensions.viewport_rows = rows;

            // Invalidate any cached rows on a resize
            inner.make_all_stale();

            let client = Arc::clone(&self.client);
            let remote_tab_id = self.remote_tab_id;
            promise::spawn::spawn(async move {
                client
                    .client
                    .resize(Resize {
                        tab_id: remote_tab_id,
                        size,
                    })
                    .await
            });
            inner.update_last_send();
        }
        Ok(())
    }

    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> anyhow::Result<()> {
        let client = Arc::clone(&self.client);
        let remote_tab_id = self.remote_tab_id;
        promise::spawn::spawn(async move {
            client
                .client
                .key_down(SendKeyDown {
                    tab_id: remote_tab_id,
                    event: KeyEvent {
                        key,
                        modifiers: mods,
                    },
                })
                .await
        });
        self.renderable
            .borrow()
            .inner
            .borrow_mut()
            .update_last_send();
        Ok(())
    }

    fn mouse_event(&self, event: MouseEvent, _host: &mut dyn TerminalHost) -> anyhow::Result<()> {
        self.mouse.borrow_mut().append(event);
        if MouseState::next(Rc::clone(&self.mouse)) {
            self.renderable
                .borrow()
                .inner
                .borrow_mut()
                .update_last_send();
        }
        Ok(())
    }

    fn advance_bytes(&self, _buf: &[u8], _host: &mut dyn TerminalHost) {
        panic!("ClientTab::advance_bytes not impl");
    }

    fn is_dead(&self) -> bool {
        self.renderable.borrow().inner.borrow().dead
    }

    fn palette(&self) -> ColorPalette {
        let config = configuration();

        if let Some(scheme_name) = config.color_scheme.as_ref() {
            if let Some(palette) = config.color_schemes.get(scheme_name) {
                return palette.clone().into();
            }
        }

        config
            .colors
            .as_ref()
            .cloned()
            .map(Into::into)
            .unwrap_or_else(ColorPalette::default)
    }

    fn domain_id(&self) -> DomainId {
        self.client.local_domain_id
    }

    fn is_mouse_grabbed(&self) -> bool {
        *self.mouse_grabbed.borrow()
    }

    fn get_current_working_dir(&self) -> Option<Url> {
        self.renderable.borrow().inner.borrow().working_dir.clone()
    }
}

struct TabWriter {
    client: Arc<ClientInner>,
    remote_tab_id: TabId,
}

impl std::io::Write for TabWriter {
    fn write(&mut self, data: &[u8]) -> Result<usize, std::io::Error> {
        promise::spawn::block_on(self.client.client.write_to_tab(WriteToTab {
            tab_id: self.remote_tab_id,
            data: data.to_vec(),
        }))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e)))?;
        Ok(data.len())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}
