use anyhow::{anyhow, Result};
use opcua::client::{ClientBuilder, IdentityToken, Session, Password};
use opcua::types::{EndpointDescription, MessageSecurityMode, UserTokenPolicy, StatusCode};
use opcua::crypto::SecurityPolicy;
use std::path::Path;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{debug, info};

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
        
        // Check if certificate authentication is required
        if let (Some(cert_path), Some(key_path)) = (self.auth_config.cert_path.clone(), self.auth_config.key_path.clone()) {
            return self.connect_with_certificate(&cert_path, &key_path).await;
        }
        
        // Regular connection without certificates
        let mut client = ClientBuilder::new()
            .application_name("OPC-UA Walker")
            .application_uri("urn:opcua-walker")
            .create_sample_keypair(false)
            .trust_server_certs(true)
            .session_retry_limit(3)
            .client()
            .map_err(|e| anyhow!("Failed to create client: {:?}", e))?;

        // Create endpoint description for anonymous/username auth
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

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn configure_certificate_auth(&self, cert_path: &str, key_path: &str) -> Result<()> {
        debug!("Validating certificate files");
        
        // Validate certificate files exist
        if !Path::new(cert_path).exists() {
            return Err(anyhow!("Certificate file not found: {}", cert_path));
        }
        if !Path::new(key_path).exists() {
            return Err(anyhow!("Private key file not found: {}", key_path));
        }

        info!("Certificate files validated successfully");
        Ok(())
    }

    async fn connect_with_certificate(&mut self, cert_path: &str, key_path: &str) -> Result<()> {
        self.configure_certificate_auth(cert_path, key_path)?;
        
        info!("🔐 Attempting certificate authentication");
        if self.verbose {
            println!("🔍 Testing certificate file compatibility...");
            println!("📄 Certificate: {} ✅", cert_path);
            println!("🔑 Private key: {} ✅", key_path);
        }

        // Create client with certificate configuration
        let mut client = ClientBuilder::new()
            .application_name("OPC-UA Walker")
            .application_uri("urn:opcua-walker")
            .certificate_path(cert_path)
            .private_key_path(key_path)
            .create_sample_keypair(false)
            .trust_server_certs(true)
            .session_retry_limit(0) // Disable retries to prevent BadTooManyOperations
            .client()
            .map_err(|e| anyhow!("Failed to create certificate client: {:?}", e))?;

        // First, discover available endpoints from the server
        debug!("Discovering server endpoints...");
        let endpoints = client
            .get_server_endpoints()
            .await
            .map_err(|e| anyhow!("Failed to discover server endpoints: {:?}", e))?;

        if self.verbose {
            println!("🔍 Discovered {} endpoint(s)", endpoints.len());
            for (i, ep) in endpoints.iter().enumerate() {
                println!("  {}. {} / {} / {}", 
                    i + 1,
                    SecurityPolicy::from_uri(ep.security_policy_uri.as_ref()),
                    ep.security_mode,
                    ep.endpoint_url
                );
            }
        }

        // Find a suitable endpoint for certificate authentication
        // Prefer stronger security policies first
        let security_policies = [
            SecurityPolicy::Aes256Sha256RsaPss,
            SecurityPolicy::Aes128Sha256RsaOaep,
            SecurityPolicy::Basic256Sha256,
            SecurityPolicy::Basic256,
            SecurityPolicy::Basic128Rsa15,
        ];

        let security_modes = [
            MessageSecurityMode::SignAndEncrypt,
            MessageSecurityMode::Sign,
        ];

        let mut chosen_endpoint = None;
        
        for policy in &security_policies {
            for mode in &security_modes {
                if let Some(endpoint) = endpoints.iter().find(|ep| {
                    SecurityPolicy::from_uri(ep.security_policy_uri.as_ref()) == *policy
                        && ep.security_mode == *mode
                }) {
                    if self.verbose {
                        println!("🔐 Selecting endpoint: {} / {}", policy, mode);
                    }
                    chosen_endpoint = Some(endpoint.clone());
                    break;
                }
            }
            if chosen_endpoint.is_some() {
                break;
            }
        }

        let endpoint = chosen_endpoint
            .ok_or_else(|| anyhow!("No suitable secure endpoint found for certificate authentication"))?;

        // Use anonymous identity token since the certificate is configured in the client
        let identity_token = IdentityToken::Anonymous;

        // Connect to the chosen endpoint
        let (session, event_loop) = client
            .connect_to_matching_endpoint(endpoint, identity_token)
            .await
            .map_err(|e| anyhow!("Certificate authentication failed: {:?}", e))?;

        // Spawn the event loop
        let handle = event_loop.spawn();

        // Wait for connection
        session.wait_for_connection().await;

        info!("✅ Certificate authentication successful");
        self.session = Some(session);
        self.event_loop_handle = Some(handle);
        
        Ok(())
    }

    fn create_identity_token(&self) -> Result<IdentityToken> {
        match (&self.auth_config.username, &self.auth_config.password) {
            (Some(username), Some(password)) => {
                debug!("Using username/password authentication");
                Ok(IdentityToken::UserName(username.clone(), Password::new(password)))
            }
            (Some(username), None) => {
                debug!("Using username authentication (no password)");
                Ok(IdentityToken::UserName(username.clone(), Password::new("")))
            }
            _ if self.auth_config.cert_path.is_some() && self.auth_config.key_path.is_some() => {
                debug!("Certificate authentication will be handled separately");
                Ok(IdentityToken::Anonymous)
            }
            _ => {
                debug!("Using anonymous authentication");
                Ok(IdentityToken::Anonymous)
            }
        }
    }
}