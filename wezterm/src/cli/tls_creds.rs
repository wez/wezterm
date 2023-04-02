use clap::Parser;
use wezterm_client::client::Client;

#[derive(Debug, Parser, Clone)]
pub struct TlsCredsCommand {}

impl TlsCredsCommand {
    pub async fn run(self, client: Client) -> anyhow::Result<()> {
        let creds = client.get_tls_creds().await?;
        codec::Pdu::GetTlsCredsResponse(creds).encode(std::io::stdout().lock(), 0)?;
        Ok(())
    }
}
