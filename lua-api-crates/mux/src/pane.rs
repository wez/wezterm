use super::*;

#[derive(Clone, Copy, Debug)]
pub struct MuxPane(pub PaneId);

impl MuxPane {
    pub fn resolve<'a>(&self, mux: &'a Rc<Mux>) -> mlua::Result<Rc<dyn Pane>> {
        mux.get_pane(self.0)
            .ok_or_else(|| mlua::Error::external(format!("pane id {} not found in mux", self.0)))
    }
}

impl UserData for MuxPane {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::ToString, |_, this, _: ()| {
            Ok(format!("MuxPane(pane_id:{}, pid:{})", this.0, unsafe {
                libc::getpid()
            }))
        });
        methods.add_method("pane_id", |_, this, _: ()| Ok(this.0));
        methods.add_async_method("split", |_, this, args: Option<SplitPane>| async move {
            let args = args.unwrap_or_default();
            args.run(this).await
        });
        methods.add_method("send_paste", |_, this, text: String| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            pane.send_paste(&text)
                .map_err(|e| mlua::Error::external(format!("{:#}", e)))?;
            Ok(())
        });
        methods.add_method("send_text", |_, this, text: String| {
            let mux = get_mux()?;
            let pane = this.resolve(&mux)?;
            pane.writer()
                .write_all(text.as_bytes())
                .map_err(|e| mlua::Error::external(format!("{:#}", e)))?;
            Ok(())
        });
        methods.add_method("window", |_, this, _: ()| {
            let mux = get_mux()?;
            Ok(mux
                .resolve_pane_id(this.0)
                .map(|(_domain_id, window_id, _tab_id)| MuxWindow(window_id)))
        });
        methods.add_method("tab", |_, this, _: ()| {
            let mux = get_mux()?;
            Ok(mux
                .resolve_pane_id(this.0)
                .map(|(_domain_id, _window_id, tab_id)| MuxTab(tab_id)))
        });
    }
}

#[derive(Debug, Default, FromDynamic, ToDynamic)]
struct SplitPane {
    #[dynamic(flatten)]
    cmd_builder: CommandBuilderFrag,
    #[dynamic(default = "spawn_tab_default_domain")]
    domain: SpawnTabDomain,
    #[dynamic(default)]
    direction: HandySplitDirection,
    #[dynamic(default)]
    top_level: bool,
    #[dynamic(default = "default_split_size")]
    size: f32,
}
impl_lua_conversion_dynamic!(SplitPane);

fn default_split_size() -> f32 {
    0.5
}

impl SplitPane {
    async fn run(self, pane: MuxPane) -> mlua::Result<MuxPane> {
        let (command, command_dir) = self.cmd_builder.to_command_builder();
        let source = SplitSource::Spawn {
            command,
            command_dir,
        };

        let size = if self.size == 0.0 {
            SplitSize::Percent(50)
        } else if self.size < 1.0 {
            SplitSize::Percent((self.size * 100.).floor() as u8)
        } else {
            SplitSize::Cells(self.size as usize)
        };

        let direction = match self.direction {
            HandySplitDirection::Right | HandySplitDirection::Left => SplitDirection::Horizontal,
            HandySplitDirection::Top | HandySplitDirection::Bottom => SplitDirection::Vertical,
        };

        let request = SplitRequest {
            direction,
            target_is_second: match self.direction {
                HandySplitDirection::Top | HandySplitDirection::Left => false,
                HandySplitDirection::Bottom | HandySplitDirection::Right => true,
            },
            top_level: self.top_level,
            size,
        };

        let mux = get_mux()?;
        let (pane, _size) = mux
            .split_pane(pane.0, request, source, self.domain)
            .await
            .map_err(|e| mlua::Error::external(format!("{:#?}", e)))?;

        Ok(MuxPane(pane.pane_id()))
    }
}
