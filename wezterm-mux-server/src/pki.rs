use anyhow::{anyhow, Context as _};
use rcgen::{BasicConstraints, Certificate, CertificateParams, DistinguishedName, DnType, IsCa};
use std::path::PathBuf;

/// A helper for managing keys for the TLS server component.
/// Each time the server is started, a new CA is generated
/// and used to generate a new server key, invalidating all
/// prior keys.
/// The idea is that the client connects via some other secure
/// channel (eg: ssh to reach the host, then unix domain to access
/// the server) to make a request for the key information.
/// We'll generate that request a new client cert and return
/// both the public CA certificate information and that key to the client.
/// The client will use both of those things to connect to the TLS
/// server.
pub struct Pki {
    ca_cert: Certificate,
    pki_dir: PathBuf,
}

impl Pki {
    pub fn init() -> anyhow::Result<Self> {
        let pki_dir = config::pki_dir()?;
        std::fs::create_dir_all(&pki_dir)?;
        log::error!("runtime dir is {}", pki_dir.display());

        let alt_names = vec![
            hostname::get()?
                .into_string()
                .map_err(|_| anyhow!("hostname is not representable as unicode"))?,
            "localhost".to_owned(),
        ];
        let unix_name = config::username_from_env()?;

        // Create the CA certificate
        let mut ca_params = CertificateParams::new(alt_names.clone());
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Constrained(1));
        ca_params.serial_number = Some(0);
        let ca_cert = Certificate::from_params(ca_params)?;
        let ca_pem = ca_cert.serialize_pem()?;
        let ca_pem_path = pki_dir.join("ca.pem");
        std::fs::write(&ca_pem_path, ca_pem.as_bytes())
            .context(format!("saving {}", ca_pem_path.display()))?;

        let mut params = CertificateParams::new(alt_names);
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, unix_name);
        params.distinguished_name = dn;

        let server_cert = Certificate::from_params(params)?;
        let mut signed_cert = server_cert.serialize_pem_with_signer(&ca_cert)?;
        let key_bits = server_cert.get_key_pair().serialize_pem();
        signed_cert.push_str(&key_bits);

        let server_pem_path = pki_dir.join("server.pem");
        std::fs::write(&server_pem_path, signed_cert.as_bytes())
            .context(format!("saving {}", server_pem_path.display()))?;

        Ok(Self { pki_dir, ca_cert })
    }

    pub fn generate_client_cert(&self) -> anyhow::Result<String> {
        let unix_name = config::username_from_env()?;

        let mut params = CertificateParams::new(vec![unix_name.clone()]);
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, unix_name);
        params.distinguished_name = dn;

        let client_cert = Certificate::from_params(params)?;
        let mut signed_cert = client_cert.serialize_pem_with_signer(&self.ca_cert)?;
        let key_bits = client_cert.get_key_pair().serialize_pem();
        signed_cert.push_str(&key_bits);

        Ok(signed_cert)
    }

    pub fn ca_pem_string(&self) -> anyhow::Result<String> {
        self.ca_cert
            .serialize_pem()
            .context("Serializing ca cert pem")
    }

    pub fn ca_pem(&self) -> PathBuf {
        self.pki_dir.join("ca.pem")
    }

    pub fn server_pem(&self) -> PathBuf {
        self.pki_dir.join("server.pem")
    }
}
