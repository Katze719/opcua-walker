use anyhow::Result;
use colored::*;
use opcua::client::Session;
use opcua::types::*;
use tabled::{Table, Tabled};
use tracing::debug;

use crate::client::OpcUaClient;

#[derive(Tabled)]
struct EndpointInfo {
    #[tabled(rename = "URL")]
    url: String,
    #[tabled(rename = "Security Policy")]
    security_policy: String,
    #[tabled(rename = "Security Mode")]
    security_mode: String,
    #[tabled(rename = "Authentication")]
    auth_tokens: String,
}

pub async fn execute(client: &mut OpcUaClient) -> Result<()> {
    let session = client.session()?;
    
    println!("\n{}", "🔍 OPC-UA Server Discovery".bright_cyan().bold());
    println!("{}", "─".repeat(50));
    
    // Get server endpoints
    debug!("Discovering server endpoints");
    let endpoints = session.get_endpoints().await?;
    
    if endpoints.is_empty() {
        println!("⚠️  No endpoints discovered");
        return Ok(());
    }
    
    let endpoint_table: Vec<EndpointInfo> = endpoints
        .into_iter()
        .map(|ep| EndpointInfo {
            url: ep.endpoint_url.to_string(),
            security_policy: format_security_policy(&ep.security_policy_uri),
            security_mode: format_security_mode(ep.security_mode),
            auth_tokens: format_user_tokens(&ep.user_identity_tokens),
        })
        .collect();
    
    println!("\n📋 {}", "Available Endpoints".bright_white().bold());
    let table = Table::new(endpoint_table);
    println!("{}", table);
    
    // Discover server applications
    debug!("Discovering server applications");
    if let Ok(applications) = session.find_servers().await {
        if !applications.is_empty() {
            println!("\n🖥️  {}", "Server Applications".bright_white().bold());
            for app in applications {
                println!("  • {} ({})", 
                    app.application_name.as_ref().bright_white(),
                    format_application_type(app.application_type).dimmed()
                );
                if !app.discovery_urls.is_empty() {
                    for url in &app.discovery_urls {
                        println!("    📡 {}", url.dimmed());
                    }
                }
            }
        }
    }
    
    println!("\n✅ {}", "Discovery completed successfully".green());
    Ok(())
}

fn format_security_policy(policy_uri: &str) -> String {
    match policy_uri {
        "http://opcfoundation.org/UA/SecurityPolicy#None" => "None".dimmed().to_string(),
        "http://opcfoundation.org/UA/SecurityPolicy#Basic128Rsa15" => "Basic128Rsa15".yellow().to_string(),
        "http://opcfoundation.org/UA/SecurityPolicy#Basic256" => "Basic256".green().to_string(),
        "http://opcfoundation.org/UA/SecurityPolicy#Basic256Sha256" => "Basic256Sha256".bright_green().to_string(),
        "http://opcfoundation.org/UA/SecurityPolicy#Aes128_Sha256_RsaOaep" => "Aes128Sha256RsaOaep".cyan().to_string(),
        "http://opcfoundation.org/UA/SecurityPolicy#Aes256_Sha256_RsaPss" => "Aes256Sha256RsaPss".bright_cyan().to_string(),
        _ => policy_uri.split('#').last().unwrap_or(policy_uri).to_string(),
    }
}

fn format_security_mode(mode: MessageSecurityMode) -> String {
    match mode {
        MessageSecurityMode::None => "None".dimmed().to_string(),
        MessageSecurityMode::Sign => "Sign".yellow().to_string(),
        MessageSecurityMode::SignAndEncrypt => "Sign+Encrypt".green().to_string(),
        _ => format!("{:?}", mode),
    }
}

fn format_user_tokens(tokens: &[UserTokenPolicy]) -> String {
    if tokens.is_empty() {
        return "None".dimmed().to_string();
    }
    
    let token_types: Vec<String> = tokens
        .iter()
        .map(|token| match token.token_type {
            UserTokenType::Anonymous => "Anonymous".to_string(),
            UserTokenType::UserName => "Username".to_string(),
            UserTokenType::Certificate => "Certificate".to_string(),
            UserTokenType::IssuedToken => "IssuedToken".to_string(),
            _ => "Unknown".to_string(),
        })
        .collect();
    
    token_types.join(", ")
}

fn format_application_type(app_type: ApplicationType) -> String {
    match app_type {
        ApplicationType::Server => "Server",
        ApplicationType::Client => "Client", 
        ApplicationType::ClientAndServer => "Client & Server",
        ApplicationType::DiscoveryServer => "Discovery Server",
        _ => "Unknown",
    }.to_string()
}