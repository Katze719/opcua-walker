use anyhow::{anyhow, Result};
use opcua::client::{ClientBuilder, IdentityToken, Session};
use opcua::types::{EndpointDescription, MessageSecurityMode, UserTokenPolicy};
use std::path::Path;
use std::sync::Arc;
use tokio::task::JoinHandle;
use opcua::types::StatusCode;
use tracing::{debug, info, warn};

use crate::types::{AuthConfig, Cli};

pub struct OpcUaClient {
    session: Option<Arc<Session>>,
    event_loop_handle: Option<JoinHandle<StatusCode>>,
    endpoint: String,
    auth_config: AuthConfig,
    verbose: bool,
}

impl OpcUaClient {
    pub async fn new(cli: &Cli) -> Result<Self> {
        Ok(Self {
            session: None,
            event_loop_handle: None,
            endpoint: cli.endpoint.clone(),
            auth_config: AuthConfig::from(cli),
            verbose: cli.verbose,
        })
    }

    pub async fn connect(&mut self) -> Result<()> {
        info!("Connecting to OPC-UA server: {}", self.endpoint);
        
        // Create client
        let mut client = ClientBuilder::new()
            .application_name("OPC-UA Walker")
            .application_uri("urn:opcua-walker")
            .create_sample_keypair(true)
            .trust_server_certs(true)
            .session_retry_limit(3)
            .client()
            .map_err(|e| anyhow!("Failed to create client: {:?}", e))?;

        // Configure certificate authentication if provided
        if let (Some(cert_path), Some(key_path)) = (&self.auth_config.cert_path, &self.auth_config.key_path) {
            self.configure_certificate_auth(cert_path, key_path)?;
        }

        // Create endpoint description
        let endpoint: EndpointDescription = (
            self.endpoint.as_str(),
            "None",
            MessageSecurityMode::None,
            UserTokenPolicy::anonymous()
        ).into();

        // Create identity token
        let identity_token = self.create_identity_token()?;

        // Connect to server
        let (session, event_loop) = client
            .connect_to_matching_endpoint(endpoint, identity_token)
            .await
            .map_err(|e| anyhow!("Failed to connect to OPC-UA server: {}", e))?;

        // Spawn the event loop
        let handle = event_loop.spawn();

        // Wait for connection
        session.wait_for_connection().await;

        info!("✅ Successfully connected to OPC-UA server");
        
        self.session = Some(session);
        self.event_loop_handle = Some(handle);
        
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(session) = self.session.take() {
            debug!("Disconnecting from OPC-UA server");
            let _ = session.disconnect().await;
            info!("✅ Disconnected from OPC-UA server");
        }
        
        if let Some(handle) = self.event_loop_handle.take() {
            handle.abort();
        }
        
        Ok(())
    }

    pub fn session(&self) -> Result<&Arc<Session>> {
        self.session.as_ref()
            .ok_or_else(|| anyhow!("Not connected to OPC-UA server"))
    }

    pub fn is_verbose(&self) -> bool {
        self.verbose
    }

    fn configure_certificate_auth(&self, cert_path: &str, key_path: &str) -> Result<()> {
        debug!("Configuring certificate authentication");
        
        // Validate certificate files exist
        if !Path::new(cert_path).exists() {
            return Err(anyhow!("Certificate file not found: {}", cert_path));
        }
        if !Path::new(key_path).exists() {
            return Err(anyhow!("Private key file not found: {}", key_path));
        }

        // TODO: Configure certificate in client - this needs to be done during ClientBuilder
        warn!("Certificate authentication configuration needs to be implemented");
        
        Ok(())
    }

    fn create_identity_token(&self) -> Result<IdentityToken> {
        match (&self.auth_config.username, &self.auth_config.password) {
            (Some(username), Some(password)) => {
                debug!("Using username/password authentication");
                Ok(IdentityToken::UserName(username.clone(), password.clone()))
            }
            (Some(username), None) => {
                debug!("Using username authentication (no password)");
                Ok(IdentityToken::UserName(username.clone(), String::new()))
            }
            _ if self.auth_config.cert_path.is_some() && self.auth_config.key_path.is_some() => {
                debug!("Using certificate authentication");
                // TODO: Certificate identity token needs to be created differently
                warn!("Certificate authentication not yet implemented");
                Ok(IdentityToken::Anonymous)
            }
            _ => {
                debug!("Using anonymous authentication");
                Ok(IdentityToken::Anonymous)
            }
        }
    }
}