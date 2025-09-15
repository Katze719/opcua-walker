use anyhow::{anyhow, Result};
use colored::*;
use opcua::client::Session;
use opcua::types::*;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, warn};

use crate::client::OpcUaClient;
use crate::utils::formatter::{format_node_id, format_node_class, truncate_string};

#[derive(Clone)]
struct TreeNode {
    reference: ReferenceDescription,
    children: Vec<TreeNode>,
    value: Option<String>,
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
    if read_values {
        println!("📖 Reading values: {}", "Enabled".bright_green());
    }
    if compact {
        println!("📦 Compact view: {}", "Enabled".bright_green());
    }
    println!("{}", "─".repeat(60));
    
    let mut visited = HashSet::new();
    
    // Build tree structure starting from the root
    let tree = build_tree_recursive(
        session,
        &start_node_id,
        0,
        max_depth,
        &mut visited,
        client.is_verbose(),
    ).await?;
    
    if tree.is_empty() {
        println!("⚠️  No nodes found");
        return Ok(());
    }
    
    // Display tree with values if requested
    display_tree(session, &tree, compact, read_values, "").await?;
    
    println!("\n✅ {}", "Browse completed successfully".green());
    Ok(())
}

async fn display_tree(
    session: &Arc<Session>,
    tree: &[TreeNode],
    compact: bool,
    read_values: bool,
    prefix: &str,
) -> Result<()> {
    for (i, node) in tree.iter().enumerate() {
        let is_last = i == tree.len() - 1;
        let current_prefix = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last { "    " } else { "│   " };
        
        // Display current node
        display_node(session, node, compact, read_values, &format!("{}{}", prefix, current_prefix)).await?;
        
        // Display children recursively
        if !node.children.is_empty() {
            Box::pin(display_tree(
                session,
                &node.children,
                compact,
                read_values,
                &format!("{}{}", prefix, child_prefix),
            )).await?;
        }
    }
    Ok(())
}

async fn display_node(
    session: &Arc<Session>,
    node: &TreeNode,
    compact: bool,
    read_values: bool,
    prefix: &str,
) -> Result<()> {
    let ref_desc = &node.reference;
    let node_id_str = format_node_id(&ref_desc.node_id.node_id);
    let display_name = &ref_desc.display_name.to_string();
    
    let value_str = if read_values && ref_desc.node_class == NodeClass::Variable {
        if let Some(cached_value) = &node.value {
            format!(" = {}", cached_value)
        } else {
            match read_node_value(session, &ref_desc.node_id.node_id).await {
                Ok(value) => format!(" = {}", value),
                Err(e) => format!(" = {}", format!("Error: {}", e).red()),
            }
        }
    } else {
        String::new()
    };
    
    if compact {
        // Compact format: prefix + class + name [node_id] = value
        println!("{}{}  {} [{}]{}",
            prefix,
            format_compact_node_class(ref_desc.node_class),
            display_name.bright_white(),
            node_id_str.dimmed(),
            value_str
        );
    } else {
        // Full format: prefix + name (class) [node_id] = value
        let type_def = if !ref_desc.type_definition.is_null() {
            format!(" <{}>", format_node_id(&ref_desc.type_definition.node_id))
        } else {
            String::new()
        };
        
        println!("{}{} ({}) [{}]{}{}",
            prefix,
            display_name.bright_white(),
            format_node_class(ref_desc.node_class),
            node_id_str.dimmed(),
            type_def.cyan(),
            value_str
        );
    }
    
    Ok(())
}

async fn build_tree_recursive(
    session: &Arc<Session>,
    node_id: &NodeId,
    current_depth: u32,
    max_depth: u32,
    visited: &mut HashSet<NodeId>,
    verbose: bool,
) -> Result<Vec<TreeNode>> {
    if current_depth > max_depth || visited.contains(node_id) {
        return Ok(Vec::new());
    }
    
    visited.insert(node_id.clone());
    
    if verbose {
        debug!("Building tree for node: {} (depth: {})", format_node_id(node_id), current_depth);
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
    
    let mut tree_nodes = Vec::new();
    
    match session.browse(&[browse_request], 0, None).await {
        Ok(browse_results) => {
            if let Some(browse_result) = browse_results.first() {
                if browse_result.status_code.is_good() {
                    if let Some(references) = &browse_result.references {
                        for reference in references {
                            let children = if current_depth < max_depth {
                                Box::pin(build_tree_recursive(
                                    session,
                                    &reference.node_id.node_id,
                                    current_depth + 1,
                                    max_depth,
                                    visited,
                                    verbose,
                                )).await.unwrap_or_else(|e| {
                                    if verbose {
                                        warn!("Failed to build tree for child {}: {}", 
                                              format_node_id(&reference.node_id.node_id), e);
                                    }
                                    Vec::new()
                                })
                            } else {
                                Vec::new()
                            };
                            
                            tree_nodes.push(TreeNode {
                                reference: reference.clone(),
                                children,
                                value: None, // Will be populated when needed
                            });
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
    
    Ok(tree_nodes)
}

async fn read_node_value(session: &Arc<Session>, node_id: &NodeId) -> Result<String> {
    match session.read(&[ReadValueId::from(node_id)], TimestampsToReturn::Both, 0.0).await {
        Ok(data_values) => {
            if let Some(data_value) = data_values.first() {
                if let Some(status) = &data_value.status {
                    if status.is_good() {
                        if let Some(value) = &data_value.value {
                            Ok(truncate_string(&format!("{}", value), 20))
                        } else {
                            Ok("null".dimmed().to_string())
                        }
                    } else {
                        Ok(format!("Error: {}", status).red().to_string())
                    }
                } else {
                    Ok("No status".dimmed().to_string())
                }
            } else {
                Ok("No data".dimmed().to_string())
            }
        }
        Err(e) => Err(anyhow::anyhow!("Read error: {}", e)),
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