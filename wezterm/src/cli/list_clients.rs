use crate::cli::CliOutputFormatKind;
use chrono::{DateTime, Utc};
use clap::Parser;
use serde::Serializer as _;
use tabout::{tabulate_output, Alignment, Column};
use wezterm_client::client::Client;

#[derive(Debug, Parser, Clone, Copy)]
pub struct ListClientsCommand {
    /// Controls the output format.
    /// "table" and "json" are possible formats.
    #[arg(long = "format", default_value = "table")]
    format: CliOutputFormatKind,
}

impl ListClientsCommand {
    pub async fn run(&self, client: Client) -> anyhow::Result<()> {
        let out = std::io::stdout();
        let clients = client.list_clients().await?;
        match self.format {
            CliOutputFormatKind::Json => {
                let clients = clients
                    .clients
                    .iter()
                    .cloned()
                    .map(CliListClientsResultItem::from);
                let mut writer = serde_json::Serializer::pretty(out.lock());
                writer.collect_seq(clients)?;
            }
            CliOutputFormatKind::Table => {
                let cols = vec![
                    Column {
                        name: "USER".to_string(),
                        alignment: Alignment::Left,
                    },
                    Column {
                        name: "HOST".to_string(),
                        alignment: Alignment::Left,
                    },
                    Column {
                        name: "PID".to_string(),
                        alignment: Alignment::Right,
                    },
                    Column {
                        name: "CONNECTED".to_string(),
                        alignment: Alignment::Left,
                    },
                    Column {
                        name: "IDLE".to_string(),
                        alignment: Alignment::Left,
                    },
                    Column {
                        name: "WORKSPACE".to_string(),
                        alignment: Alignment::Left,
                    },
                    Column {
                        name: "FOCUS".to_string(),
                        alignment: Alignment::Right,
                    },
                ];
                let mut data = vec![];
                let now: DateTime<Utc> = Utc::now();

                fn duration_string(d: chrono::Duration) -> String {
                    if let Ok(d) = d.to_std() {
                        format!("{:?}", d)
                    } else {
                        d.to_string()
                    }
                }

                for info in clients.clients {
                    let connected = now - info.connected_at;
                    let idle = now - info.last_input;
                    data.push(vec![
                        info.client_id.username.to_string(),
                        info.client_id.hostname.to_string(),
                        info.client_id.pid.to_string(),
                        duration_string(connected),
                        duration_string(idle),
                        info.active_workspace.as_deref().unwrap_or("").to_string(),
                        info.focused_pane_id
                            .map(|id| id.to_string())
                            .unwrap_or_else(String::new),
                    ]);
                }

                tabulate_output(&cols, &data, &mut out.lock())?;
            }
        }
        Ok(())
    }
}

// This will be serialized to JSON via the 'ListClients' command.
// As such it is intended to be a stable output format,
// Thus we need to be careful about the stability of the fields and types
// herein as they are directly reflected in the output.
#[derive(serde::Serialize)]
struct CliListClientsResultItem {
    username: String,
    hostname: String,
    pid: u32,
    connection_elapsed: std::time::Duration,
    idle_time: std::time::Duration,
    workspace: String,
    focused_pane_id: Option<mux::pane::PaneId>,
}

impl From<mux::client::ClientInfo> for CliListClientsResultItem {
    fn from(client_info: mux::client::ClientInfo) -> CliListClientsResultItem {
        let now: DateTime<Utc> = Utc::now();

        let mux::client::ClientInfo {
            connected_at,
            last_input,
            active_workspace,
            focused_pane_id,
            client_id,
            ..
        } = client_info;

        let mux::client::ClientId {
            username,
            hostname,
            pid,
            ..
        } = client_id.as_ref();

        let connection_elapsed = now - connected_at;
        let idle_time = now - last_input;

        CliListClientsResultItem {
            username: username.to_string(),
            hostname: hostname.to_string(),
            pid: *pid,
            connection_elapsed: connection_elapsed
                .to_std()
                .unwrap_or(std::time::Duration::ZERO),
            idle_time: idle_time.to_std().unwrap_or(std::time::Duration::ZERO),
            workspace: active_workspace.as_deref().unwrap_or("").to_string(),
            focused_pane_id: focused_pane_id,
        }
    }
}
