use anyhow::{anyhow, Result};
use colored::*;
use opcua::client::Session;
use opcua::types::*;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tracing::{debug, info};

use crate::client::OpcUaClient;
use crate::utils::formatter::{format_node_id, format_variant, format_status_code};
use crate::utils::search::find_method_with_parent;

pub async fn execute(
    client: &mut OpcUaClient,
    method_id: &str,
    object_id: Option<&str>,
    args: Option<&str>,
    verbose: bool,
) -> Result<()> {
    let session = client.session()?;
    
    println!("\n{}", "⚙️ OPC-UA Method Call".bright_cyan().bold());
    println!("{}", "─".repeat(40));
    
    // Parse method and object node IDs
    let (method_node_id, object_node_id) = if let Some(obj_id) = object_id {
        // Both method and object IDs provided
        let method_id = parse_node_id(method_id)?;
        let object_id = parse_node_id(obj_id)?;
        (method_id, object_id)
    } else if let Ok(method_node_id) = parse_node_id(method_id) {
        // Method ID provided as node ID format, need to find parent object
        info!("🔍 Finding parent object for method: {}", format_node_id(&method_node_id));
        let parent_object_id = find_parent_object(session, &method_node_id).await?;
        (method_node_id, parent_object_id)
    } else {
        // Method name provided, need to search for both method and object
        info!("🔍 Searching for method: '{}'", method_id);
        
        if let Some((method_node_id, object_node_id)) = 
            find_method_with_parent(session, method_id, verbose).await? {
            info!("✅ Found method: {} on object: {}", 
                 format_node_id(&method_node_id).bright_green(),
                 format_node_id(&object_node_id).bright_cyan());
            (method_node_id, object_node_id)
        } else {
            return Err(anyhow!("Method '{}' not found", method_id));
        }
    };
    
    // Parse input arguments
    let input_arguments = if let Some(args_str) = args {
        parse_arguments(args_str)?
    } else {
        Vec::new()
    };
    
    // Display call information
    println!("📋 {}", "Method Call Details".bright_white().bold());
    println!("   🎯 Method: {}", format_node_id(&method_node_id).bright_cyan());
    println!("   📁 Object: {}", format_node_id(&object_node_id).bright_cyan());
    if !input_arguments.is_empty() {
        println!("   📥 Arguments: {} values", input_arguments.len().to_string().bright_white());
        if verbose {
            for (i, arg) in input_arguments.iter().enumerate() {
                println!("      [{}]: {}", i, format_variant(arg));
            }
        }
    }
    
    // Execute the method call
    println!("\n⚡ Executing method call...");
    
    let call_request = CallMethodRequest {
        object_id: object_node_id.clone(),
        method_id: method_node_id.clone(),
        input_arguments: Some(input_arguments),
    };
    
    match session.call(vec![call_request]).await {
        Ok(call_results) => {
            if let Some(result) = call_results.first() {
                display_call_result(result, verbose);
            } else {
                println!("❌ No result returned from method call");
            }
        }
        Err(e) => {
            println!("❌ {}: {}", "Method call failed".red().bold(), e);
            
            // Provide troubleshooting suggestions
            println!("\n💡 {}", "Troubleshooting suggestions:".bright_yellow().bold());
            println!("   • Verify the method exists and is callable");
            println!("   • Check if the server requires authentication");
            println!("   • Ensure the object owns the specified method");
            println!("   • Try browsing the server to find available methods:");
            println!("     {}", format!("opcua-walker browse --node {}", 
                                      format_node_id(&object_node_id)).dimmed());
            
            return Err(anyhow!("Method call failed: {}", e));
        }
    }
    
    Ok(())
}

async fn find_parent_object(session: &Arc<Session>, method_node_id: &NodeId) -> Result<NodeId> {
    // Browse inverse references to find the parent object
    let browse_request = BrowseDescription {
        node_id: method_node_id.clone(),
        browse_direction: BrowseDirection::Inverse,
        reference_type_id: ReferenceTypeId::HasComponent.into(),
        include_subtypes: true,
        node_class_mask: NodeClassMask::OBJECT.bits(),
        result_mask: BrowseResultMask::All,
    };
    
    let browse_results = session.browse(&[browse_request], 0, None).await?;
    
    if let Some(browse_result) = browse_results.first() {
        if browse_result.status_code.is_good() && !browse_result.references.is_empty() {
            return Ok(browse_result.references[0].node_id.node_id.clone());
        }
    }
    
    Err(anyhow!("Could not find parent object for method: {}", 
                format_node_id(method_node_id)))
}

fn parse_arguments(args_str: &str) -> Result<Vec<Variant>> {
    let args_str = args_str.trim();
    
    // Try to parse as JSON first
    if args_str.starts_with('[') && args_str.ends_with(']') {
        return parse_json_arguments(args_str);
    }
    
    // Parse as comma-separated simple values
    if args_str.is_empty() {
        return Ok(Vec::new());
    }
    
    let values: Result<Vec<Variant>, _> = args_str
        .split(',')
        .map(|s| parse_simple_value(s.trim()))
        .collect();
    
    values
}

fn parse_json_arguments(json_str: &str) -> Result<Vec<Variant>> {
    let json_array: Vec<JsonValue> = serde_json::from_str(json_str)
        .map_err(|e| anyhow!("Failed to parse JSON arguments: {}", e))?;
    
    json_array
        .into_iter()
        .map(json_to_variant)
        .collect()
}

fn json_to_variant(json_val: JsonValue) -> Result<Variant> {
    match json_val {
        JsonValue::Null => Ok(Variant::Empty),
        JsonValue::Bool(b) => Ok(Variant::Boolean(b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                    Ok(Variant::Int32(i as i32))
                } else {
                    Ok(Variant::Int64(i))
                }
            } else if let Some(f) = n.as_f64() {
                Ok(Variant::Double(f))
            } else {
                Err(anyhow!("Invalid numeric value: {}", n))
            }
        }
        JsonValue::String(s) => Ok(Variant::String(UAString::from(s))),
        JsonValue::Array(_) => Err(anyhow!("Nested arrays not supported")),
        JsonValue::Object(_) => Err(anyhow!("Objects not supported as arguments")),
    }
}

fn parse_simple_value(value_str: &str) -> Result<Variant> {
    // Try boolean
    match value_str.to_lowercase().as_str() {
        "true" => return Ok(Variant::Boolean(true)),
        "false" => return Ok(Variant::Boolean(false)),
        _ => {}
    }
    
    // Try integer
    if let Ok(i) = value_str.parse::<i32>() {
        return Ok(Variant::Int32(i));
    }
    
    // Try float
    if let Ok(f) = value_str.parse::<f64>() {
        return Ok(Variant::Double(f));
    }
    
    // Default to string
    Ok(Variant::String(UAString::from(value_str)))
}

fn display_call_result(result: &CallMethodResult, verbose: bool) {
    println!("\n{}", "📤 Method Call Result".bright_cyan().bold());
    
    if result.status_code.is_good() {
        println!("  {}: {}", "Status".bright_white(), "✅ Success".green().bold());
        
        if !result.output_arguments.is_empty() {
            println!("  {}: {} values", "Output".bright_white(), 
                    result.output_arguments.len().to_string().bright_green());
            
            for (i, output) in result.output_arguments.iter().enumerate() {
                let value_str = format_variant(output);
                if verbose || value_str.len() <= 50 {
                    println!("    [{}]: {}", i, value_str);
                } else {
                    println!("    [{}]: {}...", i, &value_str[..47]);
                }
            }
        } else {
            println!("  {}: No return values", "Output".bright_white());
        }
    } else {
        println!("  {}: {}", "Status".bright_white(), 
                format!("❌ Failed ({})", result.status_code).red().bold());
        
        // Provide specific error guidance
        match result.status_code.name() {
            "BadMethodInvalid" => {
                println!("\n💡 The method node ID is not valid or does not reference a method");
            }
            "BadArgumentsMissing" => {
                println!("\n💡 Required arguments are missing. Use --args to provide input arguments");
            }
            "BadTooManyArguments" => {
                println!("\n💡 Too many arguments provided. Check the method signature");
            }
            "BadInvalidArgument" => {
                println!("\n💡 One or more arguments have invalid types or values");
            }
            "BadUserAccessDenied" => {
                println!("\n💡 Access denied. You may need different authentication credentials");
            }
            _ => {
                println!("\n💡 Check server logs and method requirements for more details");
            }
        }
    }
}

fn parse_node_id(node_str: &str) -> Result<NodeId> {
    NodeId::from_str(node_str)
        .map_err(|_| anyhow!("Invalid node ID format: {}", node_str))
}