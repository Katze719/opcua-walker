use anyhow::Result;
use colored::*;
use opcua::types::*;
use tracing::debug;

use crate::client::OpcUaClient;

pub async fn execute(client: &mut OpcUaClient) -> Result<()> {
    let session = client.session()?;
    
    println!("\n{}", "🔍 OPC-UA Server Information".bright_cyan().bold());
    println!("{}", "─".repeat(40));
    
    // Get server status
    let server_status_request = ReadValueId {
        node_id: VariableId::Server_ServerStatus.into(),
        attribute_id: AttributeId::Value as u32,
        index_range: NumericRange::None,
        data_encoding: QualifiedName::null(),
    };
    let server_status_results = session
        .read(&[server_status_request], TimestampsToReturn::Neither, 0.0)
        .await?;
    let server_status = server_status_results.first();
        
    if let Some(status_data) = server_status {
        println!("📊 {}: {}", "Server Status".bright_white(), 
                 format_server_status(status_data));
    }
    
    // Get server timestamp
    let current_time_request = ReadValueId {
        node_id: VariableId::Server_ServerStatus_CurrentTime.into(),
        attribute_id: AttributeId::Value as u32,
        index_range: NumericRange::None,
        data_encoding: QualifiedName::null(),
    };
    let current_time_results = session
        .read(&[current_time_request], TimestampsToReturn::Neither, 0.0)
        .await?;
        
    if let Some(current_time) = current_time_results.first() {
        if let Some(timestamp) = &current_time.value {
            println!("🕐 {}: {}", "Server Time".bright_white(), 
                     format_timestamp(timestamp));
        }
    }
    
    // Get build info
    let build_info_request = ReadValueId {
        node_id: VariableId::Server_ServerStatus_BuildInfo.into(),
        attribute_id: AttributeId::Value as u32,
        index_range: NumericRange::None,
        data_encoding: QualifiedName::null(),
    };
    let build_info_results = session
        .read(&[build_info_request], TimestampsToReturn::Neither, 0.0)
        .await?;
        
    if let Some(build_info) = build_info_results.first() {
        if let Some(build_info_value) = &build_info.value {
            println!("🏗️  {}: {}", "Build Info".bright_white(), 
                     format_build_info(build_info_value));
        }
    }
    
    // Get namespace array
    debug!("Reading namespace array");
    let namespaces_request = ReadValueId {
        node_id: VariableId::Server_NamespaceArray.into(),
        attribute_id: AttributeId::Value as u32,
        index_range: NumericRange::None,
        data_encoding: QualifiedName::null(),
    };
    let namespaces_results = session
        .read(&[namespaces_request], TimestampsToReturn::Neither, 0.0)
        .await?;
        
    if let Some(namespaces) = namespaces_results.first() {
        if let Some(Variant::Array(ns_array)) = &namespaces.value {
            println!("\n📁 {}", "Available Namespaces".bright_cyan());
            for (i, ns) in ns_array.values.iter().enumerate() {
                if let Variant::String(ns_string) = ns {
                    println!("   ns={}: {}", i, ns_string.as_ref());
                }
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