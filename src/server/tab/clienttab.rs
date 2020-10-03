use crate::server::domain::ClientInner;
use crate::server::tab::mousestate::MouseState;
use crate::server::tab::renderable::{RenderableInner, RenderableState};
use anyhow::bail;
use async_trait::async_trait;
use codec::*;
use config::configuration;
use filedescriptor::Pipe;
use log::info;
use mux::domain::DomainId;
use mux::pane::{alloc_pane_id, Pane, PaneId, Pattern, SearchResult};
use mux::renderable::{Renderable, RenderableDimensions};
use mux::tab::TabId;
use portable_pty::PtySize;
use ratelim::RateLimiter;
use std::cell::RefCell;
use std::cell::RefMut;
use std::rc::Rc;
use std::sync::Arc;
use termwiz::input::KeyEvent;
use url::Url;
use wezterm_term::color::ColorPalette;
use wezterm_term::{Clipboard, KeyCode, KeyModifiers, MouseEvent};

pub struct ClientPane {
    client: Arc<ClientInner>,
    local_pane_id: PaneId,
    pub remote_pane_id: PaneId,
    pub remote_tab_id: TabId,
    pub renderable: RefCell<RenderableState>,
    writer: RefCell<PaneWriter>,
    reader: Pipe,
    mouse: Rc<RefCell<MouseState>>,
    clipboard: RefCell<Option<Arc<dyn Clipboard>>>,
    mouse_grabbed: RefCell<bool>,
}

impl ClientPane {
    pub fn new(
        client: &Arc<ClientInner>,
        remote_tab_id: TabId,
        remote_pane_id: PaneId,
        size: PtySize,
        title: &str,
    ) -> Self {
        let local_pane_id = alloc_pane_id();
        let writer = PaneWriter {
            client: Arc::clone(client),
            remote_pane_id,
        };

        let mouse = Rc::new(RefCell::new(MouseState::new(
            remote_pane_id,
            client.client.clone(),
        )));

        let fetch_limiter =
            RateLimiter::new(|config| config.ratelimit_mux_line_prefetches_per_second);

        let render = RenderableState {
            inner: RefCell::new(RenderableInner::new(
                client,
                remote_pane_id,
                local_pane_id,
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
            remote_pane_id,
            local_pane_id,
            remote_tab_id,
            renderable: RefCell::new(render),
            writer: RefCell::new(writer),
            reader,
            clipboard: RefCell::new(None),
            mouse_grabbed: RefCell::new(false),
        }
    }

    pub fn process_unilateral(&self, pdu: Pdu) -> anyhow::Result<()> {
        match pdu {
            Pdu::GetPaneRenderChangesResponse(delta) => {
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
                        log::error!("ClientPane: Ignoring SetClipboard request {:?}", clipboard);
                    }
                }
            }
            _ => bail!("unhandled unilateral pdu: {:?}", pdu),
        };
        Ok(())
    }

    pub fn remote_pane_id(&self) -> TabId {
        self.remote_pane_id
    }
}

#[async_trait(?Send)]
impl Pane for ClientPane {
    fn pane_id(&self) -> TabId {
        self.local_pane_id
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
        let remote_pane_id = self.remote_pane_id;
        self.renderable
            .borrow()
            .inner
            .borrow_mut()
            .predict_from_paste(text);

        let data = text.to_owned();
        promise::spawn::spawn(async move {
            client
                .client
                .send_paste(SendPaste {
                    pane_id: remote_pane_id,
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
        info!("made reader for ClientPane");
        Ok(Box::new(self.reader.read.try_clone()?))
    }

    fn writer(&self) -> RefMut<dyn std::io::Write> {
        self.writer.borrow_mut()
    }

    fn set_zoomed(&self, zoomed: bool) {
        let render = self.renderable.borrow();
        let mut inner = render.inner.borrow_mut();
        let client = Arc::clone(&self.client);
        let remote_pane_id = self.remote_pane_id;
        let remote_tab_id = self.remote_tab_id;
        // Invalidate any cached rows on a resize
        inner.make_all_stale();
        promise::spawn::spawn(async move {
            client
                .client
                .set_zoomed(SetPaneZoomed {
                    containing_tab_id: remote_tab_id,
                    pane_id: remote_pane_id,
                    zoomed,
                })
                .await
        });
        inner.update_last_send();
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
            let remote_pane_id = self.remote_pane_id;
            let remote_tab_id = self.remote_tab_id;
            promise::spawn::spawn(async move {
                client
                    .client
                    .resize(Resize {
                        containing_tab_id: remote_tab_id,
                        pane_id: remote_pane_id,
                        size,
                    })
                    .await
            });
            inner.update_last_send();
        }
        Ok(())
    }

    async fn search(&self, pattern: Pattern) -> anyhow::Result<Vec<SearchResult>> {
        match self
            .client
            .client
            .search_scrollback(SearchScrollbackRequest {
                pane_id: self.remote_pane_id,
                pattern,
            })
            .await
        {
            Ok(SearchScrollbackResponse { results }) => Ok(results),
            Err(e) => Err(e),
        }
    }

    fn key_down(&self, key: KeyCode, mods: KeyModifiers) -> anyhow::Result<()> {
        let input_serial;
        {
            let renderable = self.renderable.borrow();
            let mut inner = renderable.inner.borrow_mut();
            inner.input_serial = InputSerial::now();
            input_serial = inner.input_serial;
            inner.predict_from_key_event(key, mods);
        }
        let client = Arc::clone(&self.client);
        let remote_pane_id = self.remote_pane_id;
        promise::spawn::spawn(async move {
            client
                .client
                .key_down(SendKeyDown {
                    pane_id: remote_pane_id,
                    event: KeyEvent {
                        key,
                        modifiers: mods,
                    },
                    input_serial,
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

    fn mouse_event(&self, event: MouseEvent) -> anyhow::Result<()> {
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

    fn advance_bytes(&self, _buf: &[u8]) {
        panic!("ClientPane::advance_bytes not impl");
    }

    fn is_dead(&self) -> bool {
        self.renderable.borrow().inner.borrow().dead
    }

    fn palette(&self) -> ColorPalette {
        let tardy = self.renderable.borrow().inner.borrow().is_tardy();

        let palette = || {
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
        };

        let palette = palette();
        if tardy {
            palette.grey_out()
        } else {
            palette
        }
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

struct PaneWriter {
    client: Arc<ClientInner>,
    remote_pane_id: TabId,
}

impl std::io::Write for PaneWriter {
    fn write(&mut self, data: &[u8]) -> Result<usize, std::io::Error> {
        promise::spawn::block_on(self.client.client.write_to_pane(WriteToPane {
            pane_id: self.remote_pane_id,
            data: data.to_vec(),
        }))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("{}", e)))?;
        Ok(data.len())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}
