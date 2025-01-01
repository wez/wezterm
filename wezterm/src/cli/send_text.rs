use anyhow::Context;
use clap::Parser;
use mux::pane::PaneId;
use std::io::Read;
use wezterm_client::client::Client;

#[derive(Debug, Parser, Clone)]
pub struct SendText {
    /// Specify the target pane.
    /// The default is to use the current pane based on the
    /// environment variable WEZTERM_PANE.
    #[arg(long)]
    pane_id: Option<PaneId>,

    /// Send the text directly, rather than as a bracketed paste.
    #[arg(long)]
    no_paste: bool,

    /// The text to send. If omitted, will read the text from stdin.
    text: Option<String>,
}

impl SendText {
    pub async fn run(self, client: Client) -> anyhow::Result<()> {
        let pane_id = client.resolve_pane_id(self.pane_id).await?;

        let data = match self.text {
            Some(text) => text,
            None => {
                let mut text = String::new();
                std::io::stdin()
                    .read_to_string(&mut text)
                    .context("reading stdin")?;
                text
            }
        };

        if self.no_paste {
            client
                .write_to_pane(codec::WriteToPane {
                    pane_id,
                    data: data.as_bytes().to_vec(),
                })
                .await?;
        } else {
            client
                .send_paste(codec::SendPaste { pane_id, data })
                .await?;
        }
        Ok(())
    }
}
