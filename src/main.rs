use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use opcua::client::prelude::*;
use opcua::sync::RwLock;
use opcua::types::{AttributeId, Identifier, QualifiedName, UAString};
use std::sync::Arc;
use std::str::FromStr;

#[derive(Parser)]
#[command(name = "opcua-walker")]
#[command(about = "A CLI tool for exploring OPC-UA servers and their capabilities")]
#[command(version = "0.1.0")]
struct Cli {
    /// OPC-UA Server Endpoint URL
    #[arg(short, long, default_value = "opc.tcp://localhost:4840")]
    endpoint: String,

    /// Username for authentication
    #[arg(short, long)]
    username: Option<String>,

    /// Password for authentication
    #[arg(short, long)]
    password: Option<String>,

    /// Client certificate file path for X.509 authentication
    #[arg(short, long)]
    cert: Option<String>,

    /// Client private key file path for X.509 authentication  
    #[arg(short, long)]
    key: Option<String>,

    /// Enable detailed output
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Discover server capabilities and available services
    Discover,
    /// Browse address space and show all available nodes
    Browse {
        /// Starting node for browsing (default: Objects folder)
        #[arg(short, long)]
        node: Option<String>,

        /// Maximum depth for recursive browsing
        #[arg(short, long, default_value = "3")]
        depth: u32,
    },
    /// Read node information and attributes
    Read {
        /// Node ID(s) to read (can specify multiple)
        node_ids: Vec<String>,
        
        /// Read all available attributes (default: basic info only)
        #[arg(short, long)]
        all_attributes: bool,
        
        /// Force include node value for all nodes (Variable nodes include values by default)
        #[arg(short = 'V', long)]
        include_value: bool,
    },
    /// Show server information
    Info,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    println!("{}", "🔍 OPC-UA Walker".bright_cyan().bold());
    println!("Connecting to: {}", cli.endpoint.bright_yellow());

    // Validate authentication parameters
    let auth_method = determine_auth_method(&cli)?;
    if cli.verbose {
        println!(
            "Authentication method: {}",
            format!("{:?}", auth_method).bright_blue()
        );
    }

    // Configure minimal client but with keypair generation
    let mut client = ClientBuilder::new()
        .application_name("OPC-UA Walker")
        .application_uri("urn:opcua-walker")
        .create_sample_keypair(true)
        .trust_server_certs(true)
        .client()
        .ok_or_else(|| anyhow::anyhow!("Failed to create OPC-UA client"))?;

    if cli.verbose {
        println!("Connecting to endpoint: {}", cli.endpoint);
    }

    // Create session with appropriate authentication
    let session = match auth_method {
        AuthMethod::Anonymous => {
            if cli.verbose {
                println!("Using anonymous authentication");
            }
            client.connect_to_endpoint(
                (
                    cli.endpoint.as_ref(),
                    SecurityPolicy::None.to_str(),
                    MessageSecurityMode::None,
                ),
                IdentityToken::Anonymous,
            ).map_err(|e| anyhow::anyhow!("Connection failed: {:?}", e))?
        },
        AuthMethod::UsernamePassword(username, password) => {
            if cli.verbose {
                println!("Using username/password authentication for user: {}", username);
            }
            client.connect_to_endpoint(
                (
                    cli.endpoint.as_ref(),
                    SecurityPolicy::None.to_str(),
                    MessageSecurityMode::None,
                ),
                IdentityToken::UserName(username, password),
            ).map_err(|e| anyhow::anyhow!("Connection failed: {:?}", e))?
        },
        AuthMethod::Certificate(cert_path, key_path) => {
            if cli.verbose {
                println!("Using X.509 certificate authentication");
            }
            // Use paths directly for X.509 authentication
            let cert_path_buf = std::path::PathBuf::from(&cert_path);
            let key_path_buf = std::path::PathBuf::from(&key_path);

            // Verify files exist
            if !cert_path_buf.exists() {
                return Err(anyhow::anyhow!("Certificate file not found: {}", cert_path));
            }
            if !key_path_buf.exists() {
                return Err(anyhow::anyhow!("Private key file not found: {}", key_path));
            }

            client.connect_to_endpoint(
                (
                    cli.endpoint.as_ref(),
                    SecurityPolicy::None.to_str(),
                    MessageSecurityMode::None,
                ),
                IdentityToken::X509(cert_path_buf, key_path_buf),
            ).map_err(|e| anyhow::anyhow!("Connection failed: {:?}", e))?
        }
    };

    if cli.verbose {
        println!("Successfully connected to OPC-UA server");
    }

    match &cli.command {
        Commands::Discover => {
            discover_server_capabilities(session.clone(), cli.verbose)?;
        }
        Commands::Browse { node, depth } => {
            let start_node = node.as_deref().unwrap_or("ns=0;i=85"); // Objects folder
            browse_address_space(session.clone(), start_node, *depth, cli.verbose)?;
        }
        Commands::Read { node_ids, all_attributes, include_value } => {
            read_node_information(session.clone(), node_ids, *all_attributes, *include_value, cli.verbose)?;
        }
        Commands::Info => {
            show_server_info(session.clone(), cli.verbose)?;
        }
    }

    Ok(())
}

#[derive(Debug)]
enum AuthMethod {
    Anonymous,
    UsernamePassword(String, String),
    Certificate(String, String),
}

fn determine_auth_method(cli: &Cli) -> Result<AuthMethod> {
    match (&cli.username, &cli.password, &cli.cert, &cli.key) {
        // Certificate authentication
        (None, None, Some(cert), Some(key)) => {
            Ok(AuthMethod::Certificate(cert.clone(), key.clone()))
        }
        // Username/Password authentication
        (Some(username), Some(password), None, None) => Ok(AuthMethod::UsernamePassword(
            username.clone(),
            password.clone(),
        )),
        // Anonymous authentication
        (None, None, None, None) => Ok(AuthMethod::Anonymous),
        // Invalid combinations
        (Some(_), Some(_), Some(_), Some(_)) => Err(anyhow::anyhow!(
            "Cannot use both username/password and certificate authentication simultaneously"
        )),
        (Some(_), None, None, None) => {
            Err(anyhow::anyhow!("Username provided but password is missing"))
        }
        (None, Some(_), None, None) => {
            Err(anyhow::anyhow!("Password provided but username is missing"))
        }
        (None, None, Some(_), None) => Err(anyhow::anyhow!(
            "Certificate provided but private key is missing"
        )),
        (None, None, None, Some(_)) => Err(anyhow::anyhow!(
            "Private key provided but certificate is missing"
        )),
        _ => Err(anyhow::anyhow!(
            "Invalid authentication parameter combination"
        )),
    }
}

fn discover_server_capabilities(session: Arc<RwLock<Session>>, verbose: bool) -> Result<()> {
    println!("\n{}", "🔍 Server Capabilities".bright_green().bold());

    let session_lock = session.read();

    // Test basic server connectivity and read capabilities
    let server_status_node = NodeId::new(0, 2256); // Server.ServerStatus
    let namespace_array_node = NodeId::new(0, 2255); // Server.NamespaceArray

    let read_nodes = vec![
        ReadValueId::from(&server_status_node),
        ReadValueId::from(&namespace_array_node),
    ];

    match session_lock.read(&read_nodes, TimestampsToReturn::Both, 0.0) {
        Ok(results) => {
            println!("\n{}", "📡 Server Status".bright_blue().bold());

            if let Some(result) = results.first() {
                if let Some(status) = &result.status {
                    if status.is_good() {
                        println!("  Status: {}", "Connected & Active".bright_green());
                        if verbose {
                            println!("  Status Details: {:?}", result.value);
                        }
                    } else {
                        println!("  Status: {} ({:?})", "Error".bright_red(), status);
                    }
                }
            }

            println!("\n{}", "📚 Namespace Information".bright_blue().bold());
            if let Some(result) = results.get(1) {
                if let Some(status) = &result.status {
                    if status.is_good() {
                        if let Some(value) = &result.value {
                            match value {
                                Variant::Array(array) => {
                                    println!("  Found {} namespaces:", array.values.len());
                                    for (i, item) in array.values.iter().enumerate().take(5) {
                                        match item {
                                            Variant::String(s) => {
                                                println!(
                                                    "    [{}] {}",
                                                    i,
                                                    s.to_string().bright_cyan()
                                                );
                                            }
                                            _ => {
                                                println!("    [{}] {:?}", i, item);
                                            }
                                        }
                                    }
                                    if array.values.len() > 5 {
                                        println!(
                                            "    ... and {} more namespaces",
                                            array.values.len() - 5
                                        );
                                    }
                                }
                                _ => {
                                    println!("  Namespace data: {:?}", value);
                                }
                            }
                        }
                    }
                }
            }

            println!("\n{}", "🛠️ Available Services".bright_blue().bold());
            println!("  • {} Service", "Read".bright_green());
            println!("  • {} Service", "Browse".bright_green());
            println!("  • {} Service (placeholder)", "Discovery".bright_yellow());
            if verbose {
                println!("  • Connection uses Security Policy: None");
                println!("  • Message Security Mode: None");
            }

            println!(
                "\n{}",
                "✅ Server discovery completed successfully".bright_green()
            );
        }
        Err(e) => {
            println!("{}Error reading server capabilities: {}", "❌ ".red(), e);
            return Err(anyhow::anyhow!(
                "Failed to discover server capabilities: {}",
                e
            ));
        }
    }

    Ok(())
}

fn browse_address_space(
    session: Arc<RwLock<Session>>,
    start_node: &str,
    max_depth: u32,
    verbose: bool,
) -> Result<()> {
    println!("\n{}", "📁 Address Space Browser".bright_green().bold());
    println!("Start Node: {}", start_node.bright_yellow());
    println!("Max Depth: {}", max_depth.to_string().bright_yellow());

    let session_lock = session.read();

    // Try to browse the starting node
    match browse_simple(&session_lock, start_node, max_depth, verbose) {
        Ok(count) => {
            println!("\n{}", "✅ Address space browsing completed".bright_green());
            if count > 0 {
                println!("Found {} nodes in the address space", count);
            } else {
                println!("No child nodes found from the starting point");
            }
        }
        Err(e) => {
            println!("{}Error browsing address space: {}", "❌ ".red(), e);

            // Fallback: show some standard nodes that should exist
            println!("\n{}", "📋 Standard OPC-UA Nodes".bright_blue().bold());
            println!("  These nodes should be available on any OPC-UA server:");
            println!(
                "    📁 {} (ns=0;i=85) - Root Objects folder",
                "Objects".bright_white()
            );
            println!(
                "    📄 {} (ns=0;i=2253) - Server node",
                "Server".bright_white()
            );
            println!(
                "    📊 {} (ns=0;i=2256) - Server Status",
                "ServerStatus".bright_white()
            );
            println!(
                "    📚 {} (ns=0;i=2255) - Namespace Array",
                "NamespaceArray".bright_white()
            );

            return Err(anyhow::anyhow!("Failed to browse address space: {}", e));
        }
    }

    Ok(())
}

fn browse_simple(
    session: &opcua::client::prelude::Session,
    start_node: &str,
    max_depth: u32,
    verbose: bool,
) -> Result<usize> {
    println!("\n{}", "🌳 Address Space Structure".bright_blue().bold());

    // Parse the starting node ID
    let start_node_id = parse_node_id(start_node)?;
    
    let mut found_count = 0;
    let mut nodes_to_browse = vec![(start_node_id.clone(), 0)]; // (node_id, depth)
    let mut browsed_nodes = std::collections::HashSet::new();
    
    while let Some((node_id, depth)) = nodes_to_browse.pop() {
        if depth > max_depth {
            continue;
        }
        
        // Avoid infinite loops by tracking browsed nodes
        let node_key = format!("{:?}", node_id);
        if browsed_nodes.contains(&node_key) {
            continue;
        }
        browsed_nodes.insert(node_key);

        if verbose {
            println!("📍 Browsing node: {:?} at depth {}", node_id, depth);
        }

        // Create browse request
        let browse_description = BrowseDescription {
            node_id: node_id.clone(),
            browse_direction: BrowseDirection::Forward,
            reference_type_id: ReferenceTypeId::HierarchicalReferences.into(),
            include_subtypes: true,
            node_class_mask: 0, // All node classes
            result_mask: 0x3F, // All result mask bits
        };

        match session.browse(&[browse_description]) {
            Ok(Some(results)) => {
                if let Some(result) = results.first() {
                    if result.status_code.is_good() {
                        if let Some(ref references) = result.references {
                            let indent = "  ".repeat(depth as usize);
                            
                            if depth == 0 {
                                // Show the starting node itself
                                println!("{}📁 {} ({})", 
                                    indent, 
                                    "Starting Node".bright_white(), 
                                    start_node.dimmed()
                                );
                                found_count += 1;
                            }
                            
                            for reference in references {
                                let node_class_icon = match reference.node_class {
                                    NodeClass::Object => "📁",
                                    NodeClass::Variable => "📊",
                                    NodeClass::Method => "⚡",
                                    NodeClass::ObjectType => "🏷️",
                                    NodeClass::VariableType => "🔖",
                                    NodeClass::ReferenceType => "🔗",
                                    NodeClass::DataType => "📝",
                                    NodeClass::View => "👁️",
                                    _ => "❓",
                                };
                                
                                let display_name = reference.display_name.text.as_ref();
                                
                                let node_id_str = format!("{}", reference.node_id.node_id);
                                
                                println!("{}  {} {} ({})", 
                                    indent,
                                    node_class_icon,
                                    display_name.bright_white(),
                                    node_id_str.dimmed()
                                );
                                
                                found_count += 1;
                                
                                // Add child nodes to browse queue if we haven't reached max depth
                                if depth < max_depth {
                                    nodes_to_browse.push((reference.node_id.node_id.clone(), depth + 1));
                                }
                                
                                // If it's a variable, try to read its value
                                if reference.node_class == NodeClass::Variable && verbose {
                                    if let Ok(value) = read_variable_value_sync(session, &node_id_str) {
                                        println!("{}     💠 Value: {}", 
                                            indent, 
                                            value.bright_cyan()
                                        );
                                    }
                                }
                            }
                        }
                    } else {
                        if verbose {
                            println!("❌ Browse failed for {:?}: {:?}", node_id, result.status_code);
                        }
                    }
                }
            }
            Ok(None) => {
                if verbose {
                    println!("❌ Browse returned no results for {:?}", node_id);
                }
            }
            Err(e) => {
                if verbose {
                    println!("❌ Browse error for {:?}: {}", node_id, e);
                }
            }
        }
    }

    Ok(found_count)
}

fn read_variable_value_sync(session: &opcua::client::prelude::Session, node_id: &str) -> Result<String> {
    let node = parse_node_id(node_id)?;
    let read_request = vec![ReadValueId::from(&node)];
    
    match session.read(&read_request, TimestampsToReturn::Neither, 0.0) {
        Ok(results) => {
            if let Some(result) = results.first() {
                if let Some(status) = &result.status {
                    if status.is_good() {
                        if let Some(value) = &result.value {
                            return Ok(format_value(value));
                        } else {
                            return Ok("null".to_string());
                        }
                    } else {
                        // Provide more specific error information
                        return Ok(format!("Error({})", status));
                    }
                } else {
                    return Ok("No status".to_string());
                }
            }
            Ok("No result".to_string())
        }
        Err(e) => {
            // Provide more informative error message instead of just "N/A"
            Ok(format!("ReadError: {}", e))
        }
    }
}


fn read_node_information(
    session: Arc<RwLock<Session>>, 
    node_ids: &[String], 
    all_attributes: bool,
    include_value: bool,
    verbose: bool
) -> Result<()> {
    println!("\n{}", "📖 Reading Node Information".bright_green().bold());
    
    if node_ids.is_empty() {
        return Err(anyhow::anyhow!("No node IDs provided"));
    }
    
    println!("Reading {} node(s):", node_ids.len());
    for node_id in node_ids {
        println!("  • {}", node_id.bright_yellow());
    }
    
    if all_attributes {
        println!("Mode: {}", "All Attributes".bright_cyan());
    } else {
        println!("Mode: {}", "Basic Information".bright_cyan());
    }

    let session_lock = session.read();

    for (i, node_id) in node_ids.iter().enumerate() {
        if i > 0 {
            println!("\n{}", "─".repeat(60).dimmed());
        }
        
        println!("\n{}", format!("📋 Node {} of {}", i + 1, node_ids.len()).bright_blue().bold());
        
        match read_single_node_info(&session_lock, node_id, all_attributes, include_value, verbose) {
            Ok(_) => {
                println!("  {}", "✅ Successfully read node information".bright_green());
            }
            Err(e) => {
                println!("  {}Error reading node {}: {}", "❌ ".red(), node_id, e);
                if verbose {
                    println!("     Details: {:?}", e);
                }
            }
        }
    }

    println!(
        "\n{}",
        "✅ Node reading completed".bright_green()
    );

    Ok(())
}

fn read_single_node_info(
    session: &opcua::client::prelude::Session,
    node_id: &str,
    all_attributes: bool,
    include_value: bool,
    verbose: bool,
) -> Result<()> {
    // Parse node ID
    let node = parse_node_id(node_id)?;
    
    println!("  Node ID: {}", node_id.bright_cyan());

    // First, read the node class to determine if it's a variable
    let node_class_read = vec![ReadValueId {
        node_id: node.clone(),
        attribute_id: AttributeId::NodeClass as u32,
        index_range: UAString::null(),
        data_encoding: QualifiedName::null(),
    }];
    
    let mut is_variable = false;
    match session.read(&node_class_read, TimestampsToReturn::Neither, 0.0) {
        Ok(results) => {
            if let Some(result) = results.first() {
                if let Some(status) = &result.status {
                    if status.is_good() {
                        if let Some(value) = &result.value {
                            if let Variant::Int32(class_id) = value {
                                is_variable = *class_id == 2; // NodeClass::Variable = 2
                            }
                        }
                    }
                }
            }
        }
        Err(_) => {
            // Can't determine node class, proceed without assuming it's a variable
        }
    }

    // Define which attributes to read based on mode
    let mut attributes_to_read = Vec::new();
    
    // Basic attributes (always read)
    attributes_to_read.push((AttributeId::DisplayName, "Display Name"));
    attributes_to_read.push((AttributeId::NodeClass, "Node Class"));
    attributes_to_read.push((AttributeId::BrowseName, "Browse Name"));
    
    if all_attributes || verbose {
        // Additional attributes for comprehensive reading
        attributes_to_read.push((AttributeId::Description, "Description"));
        attributes_to_read.push((AttributeId::WriteMask, "Write Mask"));
        attributes_to_read.push((AttributeId::UserWriteMask, "User Write Mask"));
        
        // Variable-specific attributes (will fail gracefully for non-variables)
        attributes_to_read.push((AttributeId::DataType, "Data Type"));
        attributes_to_read.push((AttributeId::ValueRank, "Value Rank"));
        attributes_to_read.push((AttributeId::ArrayDimensions, "Array Dimensions"));
        attributes_to_read.push((AttributeId::AccessLevel, "Access Level"));
        attributes_to_read.push((AttributeId::UserAccessLevel, "User Access Level"));
        attributes_to_read.push((AttributeId::MinimumSamplingInterval, "Min Sampling Interval"));
        attributes_to_read.push((AttributeId::Historizing, "Historizing"));
    }
    
    // Add Value attribute if explicitly requested, or if it's a variable (auto-include for variables)
    if include_value || is_variable {
        attributes_to_read.push((AttributeId::Value, "Value"));
        if is_variable && !include_value {
            println!("  {} Automatically including value for Variable node", "💡".bright_blue());
        }
    } else if !is_variable && !include_value {
        // Show a helpful message for non-variables when value is not included
        println!("  {} Use --include-value to try reading value attribute for non-Variable nodes", "💡".bright_blue());
    }

    // Build read requests
    let read_requests: Vec<ReadValueId> = attributes_to_read
        .iter()
        .map(|(attr_id, _)| ReadValueId {
            node_id: node.clone(),
            attribute_id: *attr_id as u32,
            index_range: UAString::null(),
            data_encoding: QualifiedName::null(),
        })
        .collect();

    // Execute read
    match session.read(&read_requests, TimestampsToReturn::Both, 0.0) {
        Ok(results) => {
            let mut node_class: Option<NodeClass> = None;
            
            // Process results
            for ((_attr_id, attr_name), result) in attributes_to_read.iter().zip(results.iter()) {
                if let Some(status) = &result.status {
                    if status.is_good() {
                        if let Some(value) = &result.value {
                            match attr_name {
                                &"Display Name" => {
                                    if let Variant::LocalizedText(text) = value {
                                        println!("  Display Name: {}", text.text.to_string().bright_white());
                                    }
                                }
                                &"Node Class" => {
                                    if let Variant::Int32(class_id) = value {
                                        // Convert numeric ID to NodeClass
                                        let class = match *class_id {
                                            1 => NodeClass::Object,
                                            2 => NodeClass::Variable,
                                            4 => NodeClass::Method,
                                            8 => NodeClass::ObjectType,
                                            16 => NodeClass::VariableType,
                                            32 => NodeClass::ReferenceType,
                                            64 => NodeClass::DataType,
                                            128 => NodeClass::View,
                                            _ => NodeClass::Unspecified,
                                        };
                                        node_class = Some(class);
                                        let (icon, name) = get_node_class_info(class);
                                        println!("  Node Class: {} {} ({})", icon, name.bright_white(), class_id);
                                    }
                                }
                                &"Browse Name" => {
                                    if let Variant::QualifiedName(qname) = value {
                                        println!("  Browse Name: {} (ns={})", 
                                            qname.name.to_string().bright_white(), 
                                            qname.namespace_index
                                        );
                                    }
                                }
                                &"Description" => {
                                    if let Variant::LocalizedText(text) = value {
                                        if !text.text.is_empty() {
                                            println!("  Description: {}", text.text.to_string().bright_white());
                                        }
                                    }
                                }
                                &"Data Type" => {
                                    if let Variant::NodeId(data_type_id) = value {
                                        let type_name = get_data_type_name(data_type_id);
                                        println!("  Data Type: {} ({})", type_name.bright_yellow(), data_type_id);
                                    }
                                }
                                &"Value" => {
                                    println!("  Value: {}", format_value(value).bright_white());
                                    println!("  Value Type: {}", get_variant_type_name(value).bright_yellow());
                                    
                                    // Show timestamps if available
                                    if let Some(source_ts) = &result.source_timestamp {
                                        println!(
                                            "  Source Timestamp: {}",
                                            source_ts
                                                .as_chrono()
                                                .format("%Y-%m-%d %H:%M:%S UTC")
                                                .to_string()
                                                .dimmed()
                                        );
                                    }
                                    if let Some(server_ts) = &result.server_timestamp {
                                        println!(
                                            "  Server Timestamp: {}",
                                            server_ts
                                                .as_chrono()
                                                .format("%Y-%m-%d %H:%M:%S UTC")
                                                .to_string()
                                                .dimmed()
                                        );
                                    }
                                }
                                &"Access Level" => {
                                    if let Variant::Byte(access) = value {
                                        println!("  Access Level: {} ({})", 
                                            format_access_level(*access).bright_white(), 
                                            access
                                        );
                                    }
                                }
                                &"User Access Level" => {
                                    if let Variant::Byte(access) = value {
                                        println!("  User Access Level: {} ({})", 
                                            format_access_level(*access).bright_white(), 
                                            access
                                        );
                                    }
                                }
                                &"Value Rank" => {
                                    if let Variant::Int32(rank) = value {
                                        println!("  Value Rank: {} ({})", 
                                            format_value_rank(*rank).bright_white(), 
                                            rank
                                        );
                                    }
                                }
                                &"Historizing" => {
                                    if let Variant::Boolean(hist) = value {
                                        println!("  Historizing: {}", 
                                            if *hist { "Yes".bright_green() } else { "No".dimmed() }
                                        );
                                    }
                                }
                                _ => {
                                    if verbose {
                                        println!("  {}: {}", attr_name.bright_blue(), format_value(value));
                                    }
                                }
                            }
                        }
                    } else if verbose {
                        println!("  {} (read failed): {:?}", attr_name.dimmed(), status);
                    }
                }
            }
            
            // Show additional information based on node class
            if let Some(class) = node_class {
                show_node_class_specific_info(session, &node, class, verbose)?;
            }
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to read node attributes: {}", e));
        }
    }

    Ok(())
}

fn get_node_class_info(node_class: NodeClass) -> (&'static str, &'static str) {
    match node_class {
        NodeClass::Object => ("📁", "Object"),
        NodeClass::Variable => ("📊", "Variable"),
        NodeClass::Method => ("⚡", "Method"),
        NodeClass::ObjectType => ("🏷️", "ObjectType"),
        NodeClass::VariableType => ("🔖", "VariableType"),
        NodeClass::ReferenceType => ("🔗", "ReferenceType"),
        NodeClass::DataType => ("📝", "DataType"),
        NodeClass::View => ("👁️", "View"),
        _ => ("❓", "Unknown"),
    }
}

fn get_data_type_name(node_id: &NodeId) -> String {
    // Map common OPC-UA data type node IDs to names
    match (node_id.namespace, &node_id.identifier) {
        (0, Identifier::Numeric(id)) => match *id {
            1 => "Boolean".to_string(),
            2 => "SByte".to_string(),
            3 => "Byte".to_string(),
            4 => "Int16".to_string(),
            5 => "UInt16".to_string(),
            6 => "Int32".to_string(),
            7 => "UInt32".to_string(),
            8 => "Int64".to_string(),
            9 => "UInt64".to_string(),
            10 => "Float".to_string(),
            11 => "Double".to_string(),
            12 => "String".to_string(),
            13 => "DateTime".to_string(),
            14 => "Guid".to_string(),
            15 => "ByteString".to_string(),
            16 => "XmlElement".to_string(),
            17 => "NodeId".to_string(),
            18 => "ExpandedNodeId".to_string(),
            19 => "StatusCode".to_string(),
            20 => "QualifiedName".to_string(),
            21 => "LocalizedText".to_string(),
            22 => "Structure".to_string(),
            23 => "DataValue".to_string(),
            24 => "BaseDataType".to_string(),
            25 => "DiagnosticInfo".to_string(),
            _ => format!("DataType({})", id),
        },
        _ => format!("{}", node_id),
    }
}

fn format_access_level(access: u8) -> String {
    let mut parts = Vec::new();
    if access & 0x01 != 0 { parts.push("Read"); }
    if access & 0x02 != 0 { parts.push("Write"); }
    if access & 0x04 != 0 { parts.push("HistoryRead"); }
    if access & 0x08 != 0 { parts.push("HistoryWrite"); }
    if access & 0x10 != 0 { parts.push("SemanticChange"); }
    if access & 0x20 != 0 { parts.push("StatusWrite"); }
    if access & 0x40 != 0 { parts.push("TimestampWrite"); }
    
    if parts.is_empty() {
        "None".to_string()
    } else {
        parts.join(" | ")
    }
}

fn format_value_rank(rank: i32) -> String {
    match rank {
        -3 => "ScalarOrOneDimension".to_string(),
        -2 => "Any".to_string(),
        -1 => "Scalar".to_string(),
        0 => "OneOrMoreDimensions".to_string(),
        1 => "OneDimension".to_string(),
        n if n > 1 => format!("{}Dimensions", n),
        _ => format!("Unknown({})", rank),
    }
}

fn show_node_class_specific_info(
    _session: &opcua::client::prelude::Session,
    _node_id: &NodeId,
    node_class: NodeClass,
    verbose: bool,
) -> Result<()> {
    match node_class {
        NodeClass::Object | NodeClass::ObjectType => {
            // For objects, we could show their children or type definition
            if verbose {
                println!("  {} This is an object node - use 'browse' to see its children", "💡".bright_blue());
            }
        }
        NodeClass::Method => {
            // For methods, we could show input/output arguments
            if verbose {
                println!("  {} This is a method node - can be called with appropriate arguments", "💡".bright_blue());
            }
        }
        NodeClass::Variable => {
            // Additional variable-specific information already shown above
        }
        _ => {}
    }
    Ok(())
}

fn parse_node_id(node_id: &str) -> Result<NodeId> {
    // Handle different node ID formats
    if node_id.starts_with("ns=") {
        if let Some(semicolon_pos) = node_id.find(';') {
            let ns_part = &node_id[3..semicolon_pos];
            let id_part = &node_id[semicolon_pos + 1..];

            let namespace = ns_part
                .parse::<u16>()
                .map_err(|_| anyhow::anyhow!("Invalid namespace in node ID: {}", node_id))?;

            if id_part.starts_with("i=") {
                // Numeric identifier
                let id = id_part[2..]
                    .parse::<u32>()
                    .map_err(|_| anyhow::anyhow!("Invalid numeric ID: {}", node_id))?;
                Ok(NodeId::new(namespace, id))
            } else if id_part.starts_with("s=") {
                // String identifier
                let id = id_part[2..].to_string();
                Ok(NodeId::new(namespace, id))
            } else if id_part.starts_with("g=") {
                // GUID identifier
                let guid_str = &id_part[2..];
                let guid = opcua::types::Guid::from_str(guid_str)
                    .map_err(|_| anyhow::anyhow!("Invalid GUID: {}", node_id))?;
                Ok(NodeId::new(namespace, guid))
            } else if id_part.starts_with("b=") {
                // ByteString identifier (base64 encoded)
                let bytes_str = &id_part[2..];
                use base64::Engine;
                match base64::engine::general_purpose::STANDARD.decode(bytes_str) {
                    Ok(bytes) => Ok(NodeId::new(namespace, opcua::types::ByteString::from(bytes))),
                    Err(_) => Err(anyhow::anyhow!("Invalid base64 ByteString: {}", node_id))
                }
            } else {
                Err(anyhow::anyhow!(
                    "Unsupported node ID identifier type: {}",
                    node_id
                ))
            }
        } else {
            Err(anyhow::anyhow!("Invalid node ID format: {}", node_id))
        }
    } else if node_id.starts_with("i=") {
        // Simple numeric format without namespace (assumes ns=0)
        let id = node_id[2..]
            .parse::<u32>()
            .map_err(|_| anyhow::anyhow!("Invalid numeric ID: {}", node_id))?;
        Ok(NodeId::new(0, id))
    } else if node_id.starts_with("s=") {
        // Simple string format without namespace (assumes ns=0)
        let id = node_id[2..].to_string();
        Ok(NodeId::new(0, id))
    } else {
        // Try to parse as a simple numeric value
        if let Ok(id) = node_id.parse::<u32>() {
            Ok(NodeId::new(0, id))
        } else {
            Err(anyhow::anyhow!("Unsupported node ID format: {}", node_id))
        }
    }
}

fn format_value(variant: &Variant) -> String {
    match variant {
        Variant::Boolean(b) => b.to_string(),
        Variant::SByte(i) => i.to_string(),
        Variant::Byte(u) => u.to_string(),
        Variant::Int16(i) => i.to_string(),
        Variant::UInt16(u) => u.to_string(),
        Variant::Int32(i) => i.to_string(),
        Variant::UInt32(u) => u.to_string(),
        Variant::Int64(i) => i.to_string(),
        Variant::UInt64(u) => u.to_string(),
        Variant::Float(f) => format!("{:.3}", f),
        Variant::Double(d) => format!("{:.6}", d),
        Variant::String(s) => s.to_string(),
        Variant::DateTime(dt) => dt.as_chrono().format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        Variant::LocalizedText(text) => text.text.to_string(),
        Variant::Array(array) => {
            let count = array.values.len();
            if count <= 3 {
                let items: Vec<String> = array.values.iter().map(format_value).collect();
                format!("[{}]", items.join(", "))
            } else {
                format!("Array({} items)", count)
            }
        }
        _ => format!("{:?}", variant),
    }
}

fn get_variant_type_name(variant: &Variant) -> &'static str {
    match variant {
        Variant::Boolean(_) => "Boolean",
        Variant::SByte(_) => "SByte",
        Variant::Byte(_) => "Byte",
        Variant::Int16(_) => "Int16",
        Variant::UInt16(_) => "UInt16",
        Variant::Int32(_) => "Int32",
        Variant::UInt32(_) => "UInt32",
        Variant::Int64(_) => "Int64",
        Variant::UInt64(_) => "UInt64",
        Variant::Float(_) => "Float",
        Variant::Double(_) => "Double",
        Variant::String(_) => "String",
        Variant::DateTime(_) => "DateTime",
        Variant::LocalizedText(_) => "LocalizedText",
        Variant::Array(_) => "Array",
        _ => "Unknown",
    }
}

fn show_server_info(session: Arc<RwLock<Session>>, verbose: bool) -> Result<()> {
    println!("\n{}", "ℹ️ Server Information".bright_green().bold());

    let session_lock = session.read();

    // Read server status
    let server_status_node = NodeId::new(0, 2256); // Server.ServerStatus
    let read_nodes = vec![ReadValueId::from(&server_status_node)];

    match session_lock.read(&read_nodes, TimestampsToReturn::Both, 0.0) {
        Ok(results) => {
            if let Some(result) = results.first() {
                if let Some(status) = &result.status {
                    if status.is_good() {
                        println!("Server Status: {}", "Connected".bright_green());
                        if let Some(ref value) = result.value {
                            if verbose {
                                println!("Status Code: {:?}", status);
                                println!("Value: {:?}", value);
                            }
                        }
                    } else {
                        println!("{}Server Status Error: {:?}", "❌ ".red(), status);
                    }
                }
            }
        }
        Err(e) => {
            println!("{}Error reading server information: {}", "❌ ".red(), e);
            return Err(anyhow::anyhow!("Failed to read server status: {}", e));
        }
    }

    // Read namespace array
    let namespace_array_node = NodeId::new(0, 2255); // Server.NamespaceArray
    let read_nodes = vec![ReadValueId::from(&namespace_array_node)];

    match session_lock.read(&read_nodes, TimestampsToReturn::Both, 0.0) {
        Ok(results) => {
            if let Some(result) = results.first() {
                if let Some(status) = &result.status {
                    if status.is_good() {
                        if let Some(ref value) = result.value {
                            println!("\n{}", "📚 Namespaces:".bright_blue().bold());

                            // Pretty print namespace array
                            match value {
                                Variant::Array(array) => {
                                    for (i, item) in array.values.iter().enumerate() {
                                        if let Variant::String(s) = item {
                                            println!("  [{}] {}", i, s.to_string().bright_cyan());
                                        } else {
                                            println!("  [{}] {:?}", i, item);
                                        }
                                    }
                                }
                                _ => println!("  {:?}", value),
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            if verbose {
                println!("{}Warning reading namespaces: {}", "⚠️ ".yellow(), e);
            }
        }
    }

    println!(
        "\n{}",
        "✅ Server information retrieved successfully".bright_green()
    );

    Ok(())
}
