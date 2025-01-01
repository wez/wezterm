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
                    Column {
                        name: "SSH_AUTH_SOCK".to_string(),
                        alignment: Alignment::Left,
                    },
                ];
                let mut data = vec![];
                let now: DateTime<Utc> = Utc::now();

                fn duration_string(d: chrono::Duration) -> String {
                    if let Ok(d) = d.to_std() {
                        // The default is full precision, which is a bit
                        // overwhelming (https://github.com/tailhook/humantime/issues/35).
                        // Let's auto-adjust this to be a bit more reasonable.
                        use std::time::Duration;

                        let seconds = d.as_secs();
                        let adjusted = if seconds >= 60 {
                            Duration::from_secs(seconds)
                        } else {
                            Duration::from_millis(d.as_millis() as u64)
                        };
                        let mut formatted = humantime::format_duration(adjusted).to_string();
                        formatted.retain(|c| c != ' ');
                        formatted
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
                        info.client_id
                            .ssh_auth_sock
                            .as_deref()
                            .unwrap_or("")
                            .to_string(),
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
    ssh_auth_sock: Option<String>,
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
            ssh_auth_sock,
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
            focused_pane_id,
            ssh_auth_sock: ssh_auth_sock.as_ref().map(|s| s.to_string()),
        }
    }
}
