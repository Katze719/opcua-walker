use anyhow::Result;
use colored::*;
use opcua::types::{MessageSecurityMode, UserTokenType, ApplicationType, UserTokenPolicy};
use tabled::Tabled;

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
    println!("\n{}", "🔍 OPC-UA Server Discovery".bright_cyan().bold());
    println!("{}", "─".repeat(50));
    
    println!("📡 {}: {}", "Connected Endpoint".bright_white(), 
             client.endpoint().bright_cyan());
    
    println!("✅ {}", "Discovery shows active session connection".green());
    
    // Note: Full endpoint and server discovery requires access to the Client object
    // which is not currently available after session creation.
    // This would require architectural changes to expose the Client.
    
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
    }.to_string()
}