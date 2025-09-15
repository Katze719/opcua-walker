use anyhow::{anyhow, Result};
use colored::*;
use opcua::client::Session;
use opcua::types::*;
use std::str::FromStr;
use tabled::{Table, Tabled};
use tracing::{debug, info};

use crate::client::OpcUaClient;
use crate::utils::formatter::{format_node_id, format_variant, format_status_code, format_node_class, format_access_level};
use crate::utils::search::{search_nodes_by_name, SearchConfig};

#[derive(Tabled)]
struct NodeReadInfo {
    #[tabled(rename = "Node ID")]
    node_id: String,
    #[tabled(rename = "Display Name")]
    display_name: String,
    #[tabled(rename = "Class")]
    node_class: String,
    #[tabled(rename = "Value")]
    value: String,
    #[tabled(rename = "Status")]
    status: String,
}

#[derive(Tabled)]
struct DetailedNodeInfo {
    #[tabled(rename = "Attribute")]
    attribute: String,
    #[tabled(rename = "Value")]
    value: String,
    #[tabled(rename = "Status")]
    status: String,
}

pub async fn execute(
    client: &mut OpcUaClient,
    node_ids: &[String],
    all_attributes: bool,
    include_value: bool,
    search: bool,
) -> Result<()> {
    let session = client.session()?;
    
    if node_ids.is_empty() {
        return Err(anyhow!("No node IDs provided"));
    }
    
    println!("\n{}", "📖 Reading OPC-UA Nodes".bright_cyan().bold());
    println!("{}", "─".repeat(40));
    
    let mut all_results = Vec::new();
    
    for node_str in node_ids {
        if search {
            // Search for nodes by name
            info!("🔍 Searching for nodes matching: '{}'", node_str);
            
            let config = SearchConfig {
                max_nodes: 1000,
                max_depth: 10,
                ..Default::default()
            };
            
            let search_results = search_nodes_by_name(session, node_str, config, client.is_verbose()).await?;
            
            if search_results.is_empty() {
                println!("⚠️  No nodes found matching: '{}'", node_str.yellow());
                continue;
            }
            
            println!("✅ Found {} matching nodes for '{}'", 
                    search_results.len().to_string().bright_green(), 
                    node_str.bright_white());
            
            for search_result in search_results {
                let result = read_node_info(
                    session, 
                    &search_result.node_id, 
                    all_attributes, 
                    include_value || search_result.node_class == NodeClass::Variable,
                    client.is_verbose()
                ).await?;
                all_results.push(result);
            }
        } else {
            // Read specific node ID
            let node_id = parse_node_id(node_str)?;
            debug!("Reading node: {}", format_node_id(&node_id));
            
            let result = read_node_info(
                session, 
                &node_id, 
                all_attributes, 
                include_value,
                client.is_verbose()
            ).await?;
            all_results.push(result);
        }
    }
    
    if all_results.is_empty() {
        println!("⚠️  No data retrieved");
        return Ok(());
    }
    
    // Display results
    if all_attributes {
        display_detailed_results(&all_results);
    } else {
        display_summary_results(&all_results);
    }
    
    println!("\n✅ {}", "Read operation completed successfully".green());
    Ok(())
}

async fn read_node_info(
    session: &Session,
    node_id: &NodeId,
    all_attributes: bool,
    include_value: bool,
    verbose: bool,
) -> Result<NodeData> {
    if verbose {
        debug!("Reading attributes for node: {}", format_node_id(node_id));
    }
    
    let mut attributes = vec![
        AttributeId::DisplayName,
        AttributeId::NodeClass,
        AttributeId::BrowseName,
    ];
    
    if include_value {
        attributes.push(AttributeId::Value);
    }
    
    if all_attributes {
        attributes.extend_from_slice(&[
            AttributeId::Description,
            AttributeId::DataType,
            AttributeId::ValueRank,
            AttributeId::AccessLevel,
            AttributeId::UserAccessLevel,
            AttributeId::MinimumSamplingInterval,
            AttributeId::Historizing,
        ]);
    }
    
    let read_requests: Vec<ReadValueId> = attributes
        .into_iter()
        .map(|attr| ReadValueId {
            node_id: node_id.clone(),
            attribute_id: attr as u32,
            index_range: NumericRange::None,
            data_encoding: QualifiedName::null(),
        })
        .collect();
    
    let read_results = session.read(&read_requests, TimestampsToReturn::Neither, 0.0).await?;
    
    Ok(NodeData {
        node_id: node_id.clone(),
        read_results,
        all_attributes,
    })
}

struct NodeData {
    node_id: NodeId,
    read_results: Vec<DataValue>,
    all_attributes: bool,
}

fn display_summary_results(results: &[NodeData]) {
    let table_data: Vec<NodeReadInfo> = results
        .iter()
        .map(|data| {
            let display_name = get_attribute_value(&data.read_results, 0)
                .unwrap_or_else(|| "Unknown".to_string());
            let node_class_str = get_attribute_value(&data.read_results, 1)
                .and_then(|s| s.parse::<u32>().ok())
                .and_then(|val| match val {
                    1 => Some(NodeClass::Object),
                    2 => Some(NodeClass::Variable), 
                    4 => Some(NodeClass::Method),
                    8 => Some(NodeClass::ObjectType),
                    16 => Some(NodeClass::VariableType),
                    32 => Some(NodeClass::ReferenceType),
                    64 => Some(NodeClass::DataType),
                    128 => Some(NodeClass::View),
                    _ => None,
                })
                .map(format_node_class)
                .unwrap_or_else(|| "Unknown".to_string());
            let value = get_value_string(&data.read_results, data.all_attributes);
            let status = get_status_string(&data.read_results);
            
            NodeReadInfo {
                node_id: format_node_id(&data.node_id),
                display_name,
                node_class: node_class_str,
                value,
                status,
            }
        })
        .collect();
    
    let table = Table::new(table_data);
    println!("{}", table);
}

fn display_detailed_results(results: &[NodeData]) {
    for (i, data) in results.iter().enumerate() {
        if i > 0 {
            println!();
        }
        
        println!("📋 {}: {}", "Node".bright_white(), format_node_id(&data.node_id).bright_cyan());
        
        let attributes = [
            "DisplayName", "NodeClass", "BrowseName", "Value", 
            "Description", "DataType", "ValueRank", "AccessLevel",
            "UserAccessLevel", "MinimumSamplingInterval", "Historizing"
        ];
        
        let mut table_data = Vec::new();
        
        for (idx, attr_name) in attributes.iter().enumerate() {
            if idx < data.read_results.len() {
                let data_value = &data.read_results[idx];
                let value_str = if let Some(variant) = &data_value.value {
                    match attr_name {
                        &"NodeClass" => {
                            if let Variant::UInt32(val) = variant {
                                match val {
                                    1 => Some(NodeClass::Object),
                                    2 => Some(NodeClass::Variable), 
                                    4 => Some(NodeClass::Method),
                                    8 => Some(NodeClass::ObjectType),
                                    16 => Some(NodeClass::VariableType),
                                    32 => Some(NodeClass::ReferenceType),
                                    64 => Some(NodeClass::DataType),
                                    128 => Some(NodeClass::View),
                                    _ => None,
                                }
                                .map(format_node_class)
                                .unwrap_or_else(|| format!("Unknown ({})", val))
                            } else {
                                format_variant(variant)
                            }
                        }
                        &"AccessLevel" | &"UserAccessLevel" => {
                            if let Variant::Byte(val) = variant {
                                format_access_level(*val)
                            } else {
                                format_variant(variant)
                            }
                        }
                        _ => format_variant(variant)
                    }
                } else {
                    "—".dimmed().to_string()
                };
                
                table_data.push(DetailedNodeInfo {
                    attribute: attr_name.to_string(),
                    value: value_str,
                    status: data_value.status.as_ref().map_or("Unknown".to_string(), |s| format_status_code(s)),
                });
            }
        }
        
        let table = Table::new(table_data);
        println!("{}", table);
    }
}

fn get_attribute_value(results: &[DataValue], index: usize) -> Option<String> {
    results.get(index)
        .and_then(|dv| dv.value.as_ref())
        .map(|v| match v {
            Variant::String(s) => s.to_string(),
            Variant::LocalizedText(lt) => lt.text.to_string(),
            _ => format_variant(v),
        })
}

fn get_value_string(results: &[DataValue], has_value_attr: bool) -> String {
    let value_index = if has_value_attr { 3 } else { return "—".dimmed().to_string() };
    
    results.get(value_index)
        .and_then(|dv| dv.value.as_ref())
        .map(format_variant)
        .unwrap_or_else(|| "—".dimmed().to_string())
}

fn get_status_string(results: &[DataValue]) -> String {
    if results.iter().all(|dv| dv.status.as_ref().map_or(false, |s| s.is_good())) {
        "✅ All Good".green().to_string()
    } else {
        let bad_count = results.iter().filter(|dv| !dv.status.as_ref().map_or(false, |s| s.is_good())).count();
        format!("⚠️  {} errors", bad_count).yellow().to_string()
    }
}

fn parse_node_id(node_str: &str) -> Result<NodeId> {
    NodeId::from_str(node_str)
        .map_err(|_| anyhow!("Invalid node ID format: {}", node_str))
}