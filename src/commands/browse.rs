use anyhow::{anyhow, Result};
use colored::*;
use opcua::client::Session;
use opcua::types::*;
use std::collections::HashSet;
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
}

pub async fn execute(
    client: &mut OpcUaClient,
    start_node: Option<&str>,
    max_depth: u32,
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
    
    let node_table: Vec<NodeInfo> = nodes
        .into_iter()
        .map(|node| NodeInfo {
            node_id: format_node_id(&node.node_id.node_id),
            display_name: truncate_string(&node.display_name.to_string(), 30),
            node_class: format_node_class(node.node_class),
            type_definition: if !node.type_definition.is_null() {
                Some(truncate_string(&format_node_id(&node.type_definition.node_id), 25))
            } else {
                None
            }.unwrap_or_else(|| "—".dimmed().to_string()),
        })
        .collect();
    
    println!("\n📋 {} nodes discovered", node_table.len().to_string().bright_green());
    let table = Table::new(node_table);
    println!("{}", table);
    
    println!("\n✅ {}", "Browse completed successfully".green());
    Ok(())
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
        result_mask: BrowseResultMask::All,
    };
    
    match session.browse(&[browse_request], 0, None).await {
        Ok(browse_results) => {
            if let Some(browse_result) = browse_results.first() {
                if browse_result.status_code.is_good() {
                    for reference in &browse_result.references {
                        results.push(reference.clone());
                        
                        // Recursively browse child nodes
                        if current_depth < max_depth {
                            let child_node_id = &reference.node_id.node_id;
                            if let Err(e) = browse_recursive(
                                session,
                                child_node_id,
                                current_depth + 1,
                                max_depth,
                                results,
                                visited,
                                verbose,
                            ).await {
                                if verbose {
                                    warn!("Failed to browse child node {}: {}", 
                                          format_node_id(child_node_id), e);
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