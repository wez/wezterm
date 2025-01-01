use clap::Parser;
use wezterm_client::client::Client;

#[derive(Debug, Parser, Clone)]
pub struct TlsCredsCommand {
    /// Output a PEM file encoded copy of the credentials.
    ///
    /// They will be valid for the lifetime of the mux server
    /// process.
    ///
    /// Take care with them, as anyone with them will be able
    /// to connect directly to your mux server via the network
    /// and start a shell with no additional authentication.
    #[clap(long)]
    pem: bool,
}

impl TlsCredsCommand {
    pub async fn run(self, client: Client) -> anyhow::Result<()> {
        let creds = client.get_tls_creds().await?;
        if self.pem {
            println!("{}", creds.client_cert_pem);
            // RFC 4346 says that each successive cert certifies the
            // preceeding cert, so the CA should come last
            println!("{}", creds.ca_cert_pem);
        } else {
            codec::Pdu::GetTlsCredsResponse(creds).encode(std::io::stdout().lock(), 0)?;
        }
        Ok(())
    }
}
