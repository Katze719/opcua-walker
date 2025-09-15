use anyhow::{anyhow, Result};
use colored::*;
use opcua::client::Session;
use opcua::types::*;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use tabled::{Table, Tabled};
use tracing::{debug, warn};

use crate::client::OpcUaClient;
use crate::utils::formatter::{format_node_id, format_node_class, truncate_string};

#[derive(Tabled)]
struct NodeInfo {
    #[tabled(rename = "Node ID")]
    node_id: String,
    #[tabled(rename = "Display Name")]
    display_name: String,
    #[tabled(rename = "Class")]
    node_class: String,
    #[tabled(rename = "Type")]
    type_definition: String,
    #[tabled(rename = "Value")]
    value: String,
}

#[derive(Tabled)]
struct CompactNodeInfo {
    #[tabled(rename = "ID")]
    node_id: String,
    #[tabled(rename = "Name")]
    display_name: String,
    #[tabled(rename = "Class")]
    node_class: String,
    #[tabled(rename = "Value")]
    value: String,
}

pub async fn execute(
    client: &mut OpcUaClient,
    start_node: Option<&str>,
    max_depth: u32,
    compact: bool,
    read_values: bool,
) -> Result<()> {
    let session = client.session()?;
    
    // Determine starting node
    let start_node_id = if let Some(node_str) = start_node {
        parse_node_id(node_str)?
    } else {
        ObjectId::ObjectsFolder.into()
    };
    
    println!("\n{}", "🌳 Browsing OPC-UA Address Space".bright_cyan().bold());
    println!("📍 Starting node: {}", format_node_id(&start_node_id).bright_white());
    println!("📏 Max depth: {}", max_depth.to_string().bright_white());
    println!("{}", "─".repeat(60));
    
    let mut nodes = Vec::new();
    let mut visited = HashSet::new();
    
    browse_recursive(
        session,
        &start_node_id,
        0,
        max_depth,
        &mut nodes,
        &mut visited,
        client.is_verbose(),
    ).await?;
    
    if nodes.is_empty() {
        println!("⚠️  No nodes found");
        return Ok(());
    }
    
    // Sort nodes by namespace and then by identifier for better readability
    nodes.sort_by(|a, b| {
        let ns_cmp = a.node_id.node_id.namespace.cmp(&b.node_id.node_id.namespace);
        if ns_cmp == std::cmp::Ordering::Equal {
            format_node_id(&a.node_id.node_id).cmp(&format_node_id(&b.node_id.node_id))
        } else {
            ns_cmp
        }
    });
    
    // Read values if requested
    if compact {
        display_compact_table(session, &nodes, read_values).await?;
    } else {
        display_full_table(session, &nodes, read_values).await?;
    }
    
    println!("\n✅ {}", "Browse completed successfully".green());
    Ok(())
}

async fn display_full_table(
    session: &Arc<Session>,
    nodes: &[ReferenceDescription],
    read_values: bool,
) -> Result<()> {
    let mut node_table = Vec::new();
    
    for node in nodes {
        let value = if read_values && node.node_class == NodeClass::Variable {
            read_node_value(session, &node.node_id.node_id).await
        } else {
            "—".dimmed().to_string()
        };
        
        node_table.push(NodeInfo {
            node_id: format_node_id(&node.node_id.node_id),
            display_name: truncate_string(&node.display_name.to_string(), 30),
            node_class: format_node_class(node.node_class),
            type_definition: if !node.type_definition.is_null() {
                Some(truncate_string(&format_node_id(&node.type_definition.node_id), 25))
            } else {
                None
            }.unwrap_or_else(|| "—".dimmed().to_string()),
            value,
        });
    }
    
    println!("\n📋 {} nodes discovered", node_table.len().to_string().bright_green());
    let table = Table::new(node_table);
    println!("{}", table);
    Ok(())
}

async fn display_compact_table(
    session: &Arc<Session>,
    nodes: &[ReferenceDescription],
    read_values: bool,
) -> Result<()> {
    let mut node_table = Vec::new();
    
    for node in nodes {
        let value = if read_values && node.node_class == NodeClass::Variable {
            read_node_value(session, &node.node_id.node_id).await
        } else {
            "—".dimmed().to_string()
        };
        
        node_table.push(CompactNodeInfo {
            node_id: format_node_id(&node.node_id.node_id),
            display_name: truncate_string(&node.display_name.to_string(), 20),
            node_class: format_compact_node_class(node.node_class),
            value,
        });
    }
    
    println!("\n📋 {} nodes", node_table.len().to_string().bright_green());
    let table = Table::new(node_table);
    println!("{}", table);
    Ok(())
}

async fn read_node_value(session: &Arc<Session>, node_id: &NodeId) -> String {
    match session.read(&[ReadValueId::from(node_id)], TimestampsToReturn::Both, 0.0).await {
        Ok(data_values) => {
            if let Some(data_value) = data_values.first() {
                if let Some(status) = &data_value.status {
                    if status.is_good() {
                        if let Some(value) = &data_value.value {
                            truncate_string(&format!("{}", value), 20)
                        } else {
                            "null".dimmed().to_string()
                        }
                    } else {
                        format!("Error: {}", status).red().to_string()
                    }
                } else {
                    "No status".dimmed().to_string()
                }
            } else {
                "No data".dimmed().to_string()
            }
        }
        Err(e) => format!("Read error: {}", e).red().to_string(),
    }
}

fn format_compact_node_class(node_class: NodeClass) -> String {
    match node_class {
        NodeClass::Object => "Obj".blue().to_string(),
        NodeClass::Variable => "Var".green().to_string(),
        NodeClass::Method => "Met".yellow().to_string(),
        NodeClass::ObjectType => "OTyp".cyan().to_string(),
        NodeClass::VariableType => "VTyp".magenta().to_string(),
        NodeClass::ReferenceType => "Ref".white().to_string(),
        NodeClass::DataType => "Data".bright_white().to_string(),
        NodeClass::View => "View".bright_blue().to_string(),
        _ => "?".dimmed().to_string(),
    }
}

async fn browse_recursive(
    session: &Arc<Session>,
    node_id: &NodeId,
    current_depth: u32,
    max_depth: u32,
    results: &mut Vec<ReferenceDescription>,
    visited: &mut HashSet<NodeId>,
    verbose: bool,
) -> Result<()> {
    if current_depth > max_depth || visited.contains(node_id) {
        return Ok(());
    }
    
    visited.insert(node_id.clone());
    
    if verbose {
        debug!("Browsing node: {} (depth: {})", format_node_id(node_id), current_depth);
    }
    
    // Browse the current node
    let browse_request = BrowseDescription {
        node_id: node_id.clone(),
        browse_direction: BrowseDirection::Forward,
        reference_type_id: ReferenceTypeId::HierarchicalReferences.into(),
        include_subtypes: true,
        node_class_mask: 0u32, // All node classes
        result_mask: BrowseResultMask::All as u32,
    };
    
    match session.browse(&[browse_request], 0, None).await {
        Ok(browse_results) => {
            if let Some(browse_result) = browse_results.first() {
                if browse_result.status_code.is_good() {
                    if let Some(references) = &browse_result.references {
                        for reference in references {
                            results.push(reference.clone());
                            
                            // Recursively browse child nodes
                            if current_depth < max_depth {
                                let child_node_id = &reference.node_id.node_id;
                                if let Err(e) = Box::pin(browse_recursive(
                                    session,
                                    child_node_id,
                                    current_depth + 1,
                                    max_depth,
                                    results,
                                    visited,
                                    verbose,
                                )).await {
                                    if verbose {
                                        warn!("Failed to browse child node {}: {}", 
                                              format_node_id(child_node_id), e);
                                    }
                                }
                            }
                        }
                    }
                } else if verbose {
                    warn!("Browse failed for node {}: {}", 
                          format_node_id(node_id), browse_result.status_code);
                }
            }
        }
        Err(e) if verbose => {
            warn!("Browse error for node {}: {}", format_node_id(node_id), e);
        }
        _ => {}
    }
    
    Ok(())
}

fn parse_node_id(node_str: &str) -> Result<NodeId> {
    // Try to parse as standard node ID format (ns=X;i=Y, ns=X;s=Y, etc.)
    if let Ok(node_id) = NodeId::from_str(node_str) {
        return Ok(node_id);
    }
    
    // Try common object IDs
    match node_str.to_lowercase().as_str() {
        "objects" | "objectsfolder" => Ok(ObjectId::ObjectsFolder.into()),
        "server" => Ok(ObjectId::Server.into()),
        "types" | "typesfolder" => Ok(ObjectId::TypesFolder.into()),
        "views" | "viewsfolder" => Ok(ObjectId::ViewsFolder.into()),
        "root" => Ok(ObjectId::RootFolder.into()),
        _ => Err(anyhow!("Invalid node ID format: {}", node_str)),
    }
}