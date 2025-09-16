use anyhow::Result;
use opcua::client::Session;
use opcua::types::*;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use tracing::{debug, warn};

use crate::utils::formatter::format_node_id;

pub struct SearchConfig {
    pub max_nodes: usize,
    pub max_depth: u32,
    pub search_methods_only: bool,
    pub search_variables_only: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            max_nodes: 1000,
            max_depth: 10,
            search_methods_only: false,
            search_variables_only: false,
        }
    }
}

pub struct SearchResult {
    pub node_id: NodeId,
    pub display_name: String,
    pub node_class: NodeClass,
    pub parent_node_id: Option<NodeId>,
}

pub async fn search_nodes_by_name(
    session: &Arc<Session>,
    search_name: &str,
    config: SearchConfig,
    verbose: bool,
) -> Result<Vec<SearchResult>> {
    let mut results = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    
    // Start from multiple root nodes for comprehensive search
    let start_nodes: Vec<NodeId> = vec![
        ObjectId::ObjectsFolder.into(),
        ObjectId::Server.into(),
        ObjectId::TypesFolder.into(),
    ];
    
    for start_node in start_nodes {
        queue.push_back((start_node, 0u32));
    }
    
    if verbose {
        debug!("Starting search for '{}' with max_nodes={}, max_depth={}", 
               search_name, config.max_nodes, config.max_depth);
    }
    
    let search_name_lower = search_name.to_lowercase();
    let mut nodes_processed = 0;
    
    while let Some((current_node, depth)) = queue.pop_front() {
        if nodes_processed >= config.max_nodes || depth > config.max_depth {
            break;
        }
        
        if visited.contains(&current_node) {
            continue;
        }
        visited.insert(current_node.clone());
        nodes_processed += 1;
        
        if verbose && nodes_processed % 100 == 0 {
            debug!("Processed {} nodes, queue size: {}", nodes_processed, queue.len());
        }
        
        // Browse the current node
        match browse_node(session, &current_node).await {
            Ok(references) => {
                for reference in references {
                    let node_id = &reference.node_id.node_id;
                    let display_name = reference.display_name.text.to_string();
                    
                    // Check if this node matches our search criteria
                    if should_include_node(&reference, &config) &&
                       display_name.to_lowercase().contains(&search_name_lower) {
                        results.push(SearchResult {
                            node_id: node_id.clone(),
                            display_name: display_name.clone(),
                            node_class: reference.node_class,
                            parent_node_id: Some(current_node.clone()),
                        });
                        
                        if verbose {
                            debug!("Found match: {} ({})", display_name, format_node_id(node_id));
                        }
                    }
                    
                    // Add child nodes to queue for further searching
                    if depth < config.max_depth && !visited.contains(node_id) {
                        queue.push_back((node_id.clone(), depth + 1));
                    }
                }
            }
            Err(e) if verbose => {
                warn!("Failed to browse node {}: {}", format_node_id(&current_node), e);
            }
            _ => {}
        }
    }
    
    if verbose {
        debug!("Search completed. Processed {} nodes, found {} matches", 
               nodes_processed, results.len());
    }
    
    Ok(results)
}

pub async fn find_method_with_parent(
    session: &Arc<Session>,
    method_name: &str,
    verbose: bool,
) -> Result<Option<(NodeId, NodeId)>> {
    let config = SearchConfig {
        max_nodes: 2000,
        max_depth: 15,
        search_methods_only: true,
        ..Default::default()
    };
    
    let search_results = search_nodes_by_name(session, method_name, config, verbose).await?;
    
    for result in search_results {
        if result.node_class == NodeClass::Method {
            if let Some(parent_id) = result.parent_node_id {
                return Ok(Some((result.node_id, parent_id)));
            }
        }
    }
    
    Ok(None)
}

async fn browse_node(session: &Arc<Session>, node_id: &NodeId) -> Result<Vec<ReferenceDescription>> {
    let browse_request = BrowseDescription {
        node_id: node_id.clone(),
        browse_direction: BrowseDirection::Forward,
        reference_type_id: ReferenceTypeId::HierarchicalReferences.into(),
        include_subtypes: true,
        node_class_mask: 0u32, // All node classes
        result_mask: BrowseResultMask::All as u32,
    };
    
    let browse_results = session.browse(&[browse_request], 0, None).await?;
    
    if let Some(browse_result) = browse_results.first() {
        if browse_result.status_code.is_good() {
            Ok(browse_result.references.clone().unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    } else {
        Ok(Vec::new())
    }
}

fn should_include_node(reference: &ReferenceDescription, config: &SearchConfig) -> bool {
    if config.search_methods_only {
        return reference.node_class == NodeClass::Method;
    }
    
    if config.search_variables_only {
        return reference.node_class == NodeClass::Variable;
    }
    
    // Include all node types by default
    true
}