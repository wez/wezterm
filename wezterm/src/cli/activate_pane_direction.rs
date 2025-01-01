use clap::builder::PossibleValue;
use clap::Parser;
use config::keyassignment::PaneDirection;
use mux::pane::PaneId;
use wezterm_client::client::Client;

#[derive(Debug, Parser, Clone)]
pub struct ActivatePaneDirection {
    /// Specify the current pane.
    /// The default is to use the current pane based on the
    /// environment variable WEZTERM_PANE.
    #[arg(long)]
    pane_id: Option<PaneId>,

    /// The direction to switch to.
    #[arg(value_parser=PaneDirectionParser{})]
    direction: PaneDirection,
}

impl ActivatePaneDirection {
    pub async fn run(&self, client: Client) -> anyhow::Result<()> {
        let pane_id = client.resolve_pane_id(self.pane_id).await?;
        client
            .activate_pane_direction(codec::ActivatePaneDirection {
                pane_id,
                direction: self.direction,
            })
            .await?;
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub struct PaneDirectionParser {}

impl clap::builder::TypedValueParser for PaneDirectionParser {
    type Value = PaneDirection;

    fn parse_ref(
        &self,
        _cmd: &clap::Command,
        _arg: Option<&clap::Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, clap::Error> {
        use clap::error::*;

        let value = value
            .to_str()
            .ok_or_else(|| Error::raw(ErrorKind::InvalidUtf8, "value must be a utf8 string\n"))?;
        PaneDirection::direction_from_str(value)
            .map_err(|e| Error::raw(ErrorKind::InvalidValue, format!("{e}\n")))
    }

    fn possible_values(&self) -> Option<Box<dyn Iterator<Item = PossibleValue>>> {
        Some(Box::new(
            PaneDirection::variants().iter().map(PossibleValue::new),
        ))
    }
}
