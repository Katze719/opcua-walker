use anyhow::Result;
use colored::*;
use opcua::client::Session;
use opcua::types::*;
use tracing::debug;

use crate::client::OpcUaClient;

pub async fn execute(client: &mut OpcUaClient) -> Result<()> {
    let session = client.session()?;
    
    println!("\n{}", "🔍 OPC-UA Server Information".bright_cyan().bold());
    println!("{}", "─".repeat(40));
    
    // Get server status
    let server_status = session
        .read(&ReadValueId::from(VariableId::Server_ServerStatus))
        .await?;
        
    println!("📊 {}: {}", "Server Status".bright_white(), 
             format_server_status(&server_status));
    
    // Get server timestamp
    let current_time = session
        .read(&ReadValueId::from(VariableId::Server_ServerStatus_CurrentTime))
        .await?;
        
    if let Some(timestamp) = current_time.value {
        println!("🕐 {}: {}", "Server Time".bright_white(), 
                 format_timestamp(&timestamp));
    }
    
    // Get build info
    let build_info = session
        .read(&ReadValueId::from(VariableId::Server_ServerStatus_BuildInfo))
        .await?;
        
    if let Some(build_info) = build_info.value {
        println!("🏗️  {}: {}", "Build Info".bright_white(), 
                 format_build_info(&build_info));
    }
    
    // Get namespace array
    debug!("Reading namespace array");
    let namespaces = session
        .read(&ReadValueId::from(VariableId::Server_NamespaceArray))
        .await?;
        
    if let Some(Variant::Array(ns_array)) = namespaces.value {
        println!("\n📁 {}", "Available Namespaces".bright_cyan());
        for (i, ns) in ns_array.values.iter().enumerate() {
            if let Variant::String(ns_string) = ns {
                println!("   ns={}: {}", i, ns_string.as_ref());
            }
        }
    }
    
    println!("\n✅ {}", "Server information retrieved successfully".green());
    Ok(())
}

fn format_server_status(status: &DataValue) -> String {
    if let Some(Variant::UInt32(status_code)) = &status.value {
        match *status_code {
            0 => "Running".green().to_string(),
            1 => "Failed".red().to_string(),
            2 => "No Configuration".yellow().to_string(),
            3 => "Suspended".yellow().to_string(),
            4 => "Shutdown".red().to_string(),
            5 => "Test".blue().to_string(),
            6 => "Communication Fault".red().to_string(),
            7 => "Unknown".dimmed().to_string(),
            _ => format!("Unknown ({})", status_code).dimmed().to_string(),
        }
    } else {
        "Unknown".dimmed().to_string()
    }
}

fn format_timestamp(timestamp: &Variant) -> String {
    if let Variant::DateTime(dt) = timestamp {
        dt.as_chrono().format("%Y-%m-%d %H:%M:%S UTC").to_string()
    } else {
        "Unknown".dimmed().to_string()
    }
}

fn format_build_info(build_info: &Variant) -> String {
    if let Variant::ExtensionObject(ext_obj) = build_info {
        // Try to extract build info fields
        format!("Build information available ({})", ext_obj.node_id)
    } else {
        "Not available".dimmed().to_string()
    }
}