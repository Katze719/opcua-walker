use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use opcua::client::prelude::*;
use opcua::sync::RwLock;
use opcua::types::{AttributeId, Identifier, QualifiedName, UAString, CallMethodRequest};
use std::sync::Arc;
use std::str::FromStr;
use std::collections::HashSet;

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
        /// Node ID(s) to read (can specify multiple) or name to search for
        node_ids: Vec<String>,
        
        /// Read all available attributes (default: basic info only)
        #[arg(short, long)]
        all_attributes: bool,
        
        /// Force include node value for all nodes (Variable nodes include values by default)
        #[arg(short = 'V', long)]
        include_value: bool,
        
        /// Search for nodes by display name instead of using exact node ID
        #[arg(short, long)]
        search: bool,
    },
    /// Call a method on the server
    Call {
        /// Method name or node ID to call
        method_id: String,
        
        /// Object node ID that owns the method (optional - will auto-search if not provided)
        object_id: Option<String>,
        
        /// Input arguments for the method (JSON format or simple values)
        #[arg(short, long)]
        args: Option<String>,
        
        /// Show detailed call information
        #[arg(short, long)]
        verbose: bool,
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

    // Configure client with more tolerant settings for various servers
    let mut client = ClientBuilder::new()
        .application_name("OPC-UA Walker")
        .application_uri("urn:opcua-walker")
        .create_sample_keypair(true)
        .trust_server_certs(true)
        .session_retry_limit(1)
        .session_retry_interval(1000)
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

            // Validate certificate file extensions
            if let Some(cert_extension) = cert_path_buf.extension() {
                let ext = cert_extension.to_string_lossy().to_lowercase();
                if !["pem", "der"].contains(&ext.as_str()) {
                    if cli.verbose {
                        println!("  ⚠️  Warning: Certificate file extension '{}' is uncommon. Supported: .pem (text), .der (binary)", ext);
                        println!("      Note: .crt and .cer files may work if they contain PEM or DER data");
                    }
                }
                if cli.verbose {
                    let format_type = match ext.as_str() {
                        "der" => "binary DER format",
                        "pem" => "text PEM format", 
                        _ => "unknown format"
                    };
                    println!("  📄 Certificate file: {} ({}, {})", cert_path, ext, format_type);
                }
            }

            // Validate key file extensions  
            if let Some(key_extension) = key_path_buf.extension() {
                let ext = key_extension.to_string_lossy().to_lowercase();
                if !["pem", "key"].contains(&ext.as_str()) {
                    if cli.verbose {
                        println!("  ⚠️  Warning: Private key file extension '{}' is uncommon. Supported: .pem, .key (both in PEM text format)", ext);
                        println!("      Note: DER format private keys are not supported by the OPC-UA library");
                    }
                }
                if cli.verbose {
                    println!("  🔑 Private key file: {} ({}, PEM text format)", key_path, ext);
                }
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
        Commands::Read { node_ids, all_attributes, include_value, search } => {
            if *search {
                read_nodes_by_search(session.clone(), node_ids, *all_attributes, *include_value, cli.verbose)?;
            } else {
                read_node_information(session.clone(), node_ids, *all_attributes, *include_value, cli.verbose)?;
            }
        }
        Commands::Call { method_id, object_id, args, verbose } => {
            call_method(session.clone(), method_id, object_id.as_deref(), args.as_deref(), *verbose || cli.verbose)?;
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
                    // According to OPC-UA spec, None status typically means Good
                    if let Some(value) = &result.value {
                        return Ok(format_value(value));
                    } else {
                        return Ok("null".to_string());
                    }
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
                } else {
                    // Handle None status according to OPC-UA spec (treat as Good)
                    if let Some(value) = &result.value {
                        // Just handle the Value attribute for now since that's what the user cares about
                        if *attr_name == "Value" {
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
                        } else if verbose {
                            // For other attributes, use the existing format_value function
                            println!("  {}: {}", attr_name.bright_blue(), format_value(value));
                        }
                    } else {
                        // Value is None - show appropriate message
                        if *attr_name == "Value" {
                            println!("  Value: {}", "null".dimmed());
                        } else if verbose {
                            println!("  {}: {}", attr_name.dimmed(), "null".dimmed());
                        }
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

fn call_method(
    session: Arc<RwLock<Session>>,
    method_id: &str,
    object_id: Option<&str>,
    args: Option<&str>,
    verbose: bool,
) -> Result<()> {
    println!("\n{}", "⚡ Calling Method".bright_green().bold());
    println!("Method: {}", method_id.bright_yellow());
    
    let session_lock = session.read();
    
    // Determine if we need to search for the method or use provided IDs
    let (method_node, object_node) = if let Some(object_id_str) = object_id {
        // Both method and object IDs provided - use directly
        println!("Object ID: {}", object_id_str.bright_yellow());
        (parse_node_id(method_id)?, parse_node_id(object_id_str)?)
    } else {
        // Only method name provided - try different search strategies
        println!("Object ID: {}", "Auto-searching...".dimmed());
        
        // First, try to interpret method_id as a node ID in case user provided one
        if method_id.contains("ns=") && method_id.contains(";") {
            println!("  💡 Method ID looks like a node ID, trying direct parsing...");
            match parse_node_id(method_id) {
                Ok(method_node_id) => {
                    println!("  ✅ Successfully parsed as node ID: {}", format!("{}", method_node_id).dimmed());
                    println!("  ⚠️  Warning: No object ID provided, using same node as both method and object");
                    println!("     This might not work. Consider providing object ID explicitly:");
                    println!("     opcua-walker call \"{}\" \"<object-node-id>\"", method_id);
                    (method_node_id.clone(), method_node_id)
                }
                Err(_) => {
                    println!("  ⚠️  Failed to parse as node ID, falling back to name search...");
                    search_for_method(&session_lock, method_id, verbose)?
                }
            }
        } else {
            // Regular name-based search
            search_for_method(&session_lock, method_id, verbose)?
        }
    };
    
    if let Some(args_str) = args {
        println!("Input Arguments: {}", args_str.bright_blue());
    } else {
        println!("Input Arguments: {}", "None".dimmed());
    }

    // Parse input arguments
    let input_args = if let Some(args_str) = args {
        parse_method_arguments(args_str)?
    } else {
        Vec::new()
    };

    if verbose {
        println!("Parsed method node: {:?}", method_node);
        println!("Parsed object node: {:?}", object_node);
        println!("Parsed input arguments: {:?}", input_args);
    }

    // Create method call request
    let call_request = CallMethodRequest {
        object_id: object_node,
        method_id: method_node,
        input_arguments: Some(input_args),
    };

    // Execute method call
    match session_lock.call(call_request) {
        Ok(result) => {
            println!("\n{}", "📤 Method Call Result".bright_blue().bold());
            
            let status = &result.status_code;
            if status.is_good() {
                println!("  Status: {}", "Success".bright_green());
                
                // Display output arguments
                if let Some(ref output_args) = result.output_arguments {
                    if !output_args.is_empty() {
                        println!("  Output Arguments:");
                        for (i, arg) in output_args.iter().enumerate() {
                            println!("    [{}] {} ({})", 
                                i, 
                                format_value(arg).bright_white(), 
                                get_variant_type_name(arg).bright_yellow()
                            );
                        }
                    } else {
                        println!("  Output Arguments: {}", "None".dimmed());
                    }
                } else {
                    println!("  Output Arguments: {}", "None".dimmed());
                }
                
                println!("  {}", "✅ Method call completed successfully".bright_green());
            } else {
                println!("  Status: {} ({:?})", "Failed".bright_red(), status);
                
                // Provide specific guidance for common error codes
                let status_str = format!("{:?}", status);
                if status_str.contains("BadTooManyOperations") {
                    println!("  💡 {}", "Suggestion: The server may be overwhelmed by too many browse operations.".bright_yellow());
                    println!("     Try using the exact node ID instead: opcua-walker call \"ns=X;s=MethodName\" \"ns=X;s=ObjectName\"");
                }
                if status_str.contains("BadLicenseNotAvailable") {
                    println!("  💡 {}", "Suggestion: The server may require a license for method execution.".bright_yellow());
                    println!("     Check server licensing requirements or contact server administrator.");
                }
                if status_str.contains("BadUnexpectedError") {
                    println!("  💡 {}", "Suggestion: The method may require specific arguments or have execution restrictions.".bright_yellow());
                    println!("     Try calling with explicit arguments: --args \"[arg1, arg2]\" or check method signature.");
                }
                
                // Still try to display output arguments for diagnostic purposes
                if let Some(ref output_args) = result.output_arguments {
                    if !output_args.is_empty() {
                        println!("  Output Arguments (diagnostic):");
                        for (i, arg) in output_args.iter().enumerate() {
                            println!("    [{}] {} ({})", 
                                i, 
                                format_value(arg).bright_white(), 
                                get_variant_type_name(arg).bright_yellow()
                            );
                        }
                    }
                }
                
                return Err(anyhow::anyhow!("Method call failed with status: {:?}", status));
            }
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Method call failed: {}", e));
        }
    }

    Ok(())
}

fn parse_method_arguments(args_str: &str) -> Result<Vec<Variant>> {
    let mut arguments = Vec::new();
    
    // Try to parse as JSON first
    if args_str.trim().starts_with('[') {
        match serde_json::from_str::<Vec<serde_json::Value>>(args_str) {
            Ok(json_args) => {
                for json_arg in json_args {
                    arguments.push(json_value_to_variant(json_arg)?);
                }
                return Ok(arguments);
            }
            Err(_) => {
                // Fall through to simple parsing
            }
        }
    }
    
    // Simple comma-separated parsing
    if args_str.contains(',') {
        for arg in args_str.split(',') {
            arguments.push(parse_simple_argument(arg.trim())?);
        }
    } else if !args_str.trim().is_empty() {
        arguments.push(parse_simple_argument(args_str.trim())?);
    }
    
    Ok(arguments)
}

fn json_value_to_variant(value: serde_json::Value) -> Result<Variant> {
    match value {
        serde_json::Value::Bool(b) => Ok(Variant::Boolean(b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                if i >= i32::MIN as i64 && i <= i32::MAX as i64 {
                    Ok(Variant::Int32(i as i32))
                } else {
                    Ok(Variant::Int64(i))
                }
            } else if let Some(f) = n.as_f64() {
                Ok(Variant::Double(f))
            } else {
                Err(anyhow::anyhow!("Invalid number format"))
            }
        }
        serde_json::Value::String(s) => Ok(Variant::String(UAString::from(s))),
        serde_json::Value::Array(arr) => {
            let mut variants = Vec::new();
            for item in arr {
                variants.push(json_value_to_variant(item)?);
            }
            // Create array with unknown type - let OPC-UA figure it out
            match opcua::types::Array::new(opcua::types::VariantTypeId::Empty, variants) {
                Ok(array) => Ok(Variant::Array(Box::new(array))),
                Err(e) => Err(anyhow::anyhow!("Failed to create array: {:?}", e))
            }
        }
        serde_json::Value::Null => Ok(Variant::Empty),
        _ => Err(anyhow::anyhow!("Unsupported JSON value type")),
    }
}

fn parse_simple_argument(arg: &str) -> Result<Variant> {
    // Try boolean
    if arg.eq_ignore_ascii_case("true") {
        return Ok(Variant::Boolean(true));
    }
    if arg.eq_ignore_ascii_case("false") {
        return Ok(Variant::Boolean(false));
    }
    
    // Try integer
    if let Ok(i) = arg.parse::<i32>() {
        return Ok(Variant::Int32(i));
    }
    
    // Try float
    if let Ok(f) = arg.parse::<f64>() {
        return Ok(Variant::Double(f));
    }
    
    // Default to string
    Ok(Variant::String(UAString::from(arg)))
}

fn read_nodes_by_search(
    session: Arc<RwLock<Session>>,
    search_terms: &[String],
    all_attributes: bool,
    include_value: bool,
    verbose: bool,
) -> Result<()> {
    println!("\n{}", "🔍 Searching for Nodes".bright_green().bold());
    
    if search_terms.is_empty() {
        return Err(anyhow::anyhow!("No search terms provided"));
    }
    
    println!("Search terms:");
    for term in search_terms {
        println!("  • {}", term.bright_yellow());
    }

    let session_lock = session.read();
    
    let mut all_found_nodes = Vec::new();
    
    for search_term in search_terms {
        println!("\n{}", format!("🔎 Searching for: {}", search_term).bright_blue().bold());
        
        let found_nodes = search_nodes_by_name(&session_lock, search_term, verbose)?;
        
        if found_nodes.is_empty() {
            println!("  {} No nodes found matching '{}'", "❌".red(), search_term);
        } else {
            println!("  {} Found {} matching node(s):", "✅".bright_green(), found_nodes.len());
            
            for (i, (node_id, display_name, node_class)) in found_nodes.iter().enumerate() {
                let (icon, class_name) = get_node_class_info(*node_class);
                println!("    [{}] {} {} {} ({})", 
                    i + 1,
                    icon,
                    display_name.bright_white(), 
                    class_name.dimmed(),
                    format!("{}", node_id).dimmed()
                );
            }
            
            all_found_nodes.extend(found_nodes);
        }
    }
    
    if all_found_nodes.is_empty() {
        println!("\n{} No nodes found for any search terms", "❌".red());
        return Ok(());
    }
    
    // Read information for all found nodes
    println!("\n{}", "📖 Reading Node Information".bright_blue().bold());
    
    for (i, (node_id, display_name, _node_class)) in all_found_nodes.iter().enumerate() {
        if i > 0 {
            println!("\n{}", "─".repeat(60).dimmed());
        }
        
        println!("\n{}", format!("📋 Node: {} ({})", display_name, node_id).bright_blue().bold());
        
        let node_id_str = format!("{}", node_id);
        match read_single_node_info(&session_lock, &node_id_str, all_attributes, include_value, verbose) {
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
        format!("✅ Search and read completed - {} nodes processed", all_found_nodes.len()).bright_green()
    );
    
    Ok(())
}

fn search_nodes_by_name(
    session: &opcua::client::prelude::Session,
    search_term: &str,
    verbose: bool,
) -> Result<Vec<(NodeId, String, NodeClass)>> {
    // Use progressive search strategy for better performance
    // Start with fast shallow search, then go deeper only if needed
    
    if verbose {
        println!("    🎯 Starting progressive search for: '{}'", search_term);
    }
    
    // Stage 1: Quick search (depth-limited like browse)
    let quick_results = search_nodes_with_depth_limit(session, search_term, 3, verbose)?;
    if !quick_results.is_empty() {
        if verbose {
            println!("    ✅ Quick search found {} matches", quick_results.len());
        }
        return Ok(quick_results);
    }
    
    // Stage 2: Broader search if no exact matches found
    if verbose {
        println!("    🔄 No quick matches, trying broader search...");
    }
    let broader_results = search_nodes_with_depth_limit(session, search_term, 5, verbose)?;
    if !broader_results.is_empty() {
        if verbose {
            println!("    ✅ Broader search found {} matches", broader_results.len());
        }
        return Ok(broader_results);
    }
    
    // Stage 3: Deep search as last resort
    if verbose {
        println!("    🔍 No matches yet, trying deep search...");
    }
    search_nodes_deep(session, search_term, verbose)
}

fn search_nodes_with_depth_limit(
    session: &opcua::client::prelude::Session,
    search_term: &str,
    max_depth: u32,
    verbose: bool,
) -> Result<Vec<(NodeId, String, NodeClass)>> {
    let mut found_nodes = Vec::new();
    let mut nodes_to_browse = vec![
        (NodeId::new(0, 85), 0),   // Objects folder
        (NodeId::new(0, 2253), 0), // Server node  
        (NodeId::new(0, 86), 0),   // Types folder
    ];
    let mut browsed_nodes = HashSet::new();
    
    while let Some((node_id, depth)) = nodes_to_browse.pop() {
        if depth > max_depth {
            continue;
        }
        
        let node_key = format!("{:?}", node_id);
        if browsed_nodes.contains(&node_key) {
            continue;
        }
        browsed_nodes.insert(node_key);
        
        // Create browse request
        let browse_description = BrowseDescription {
            node_id: node_id.clone(),
            browse_direction: BrowseDirection::Forward,
            reference_type_id: ReferenceTypeId::HierarchicalReferences.into(),
            include_subtypes: true,
            node_class_mask: 0, // All node classes
            result_mask: 0x3F, // All result mask bits
        };
        
        // No artificial delays - let the server respond at its natural speed
        if let Ok(Some(results)) = session.browse(&[browse_description]) {
            if let Some(result) = results.first() {
                if result.status_code.is_good() {
                    if let Some(ref references) = result.references {
                        for reference in references {
                            let display_name = reference.display_name.text.as_ref();
                            let browse_name = &reference.browse_name.name;
                            
                            // Check for exact matches first, then partial matches
                            let exact_display_match = display_name.eq_ignore_ascii_case(search_term);
                            let exact_browse_match = browse_name.to_string().eq_ignore_ascii_case(search_term);
                            let partial_display_match = display_name.to_lowercase().contains(&search_term.to_lowercase());
                            let partial_browse_match = browse_name.to_string().to_lowercase().contains(&search_term.to_lowercase());
                            
                            if exact_display_match || exact_browse_match || partial_display_match || partial_browse_match {
                                found_nodes.push((
                                    reference.node_id.node_id.clone(),
                                    display_name.to_string(),
                                    reference.node_class,
                                ));
                                
                                if verbose {
                                    println!("    ✅ Found: '{}' (ns={}, depth={}, class: {:?})", 
                                        display_name, reference.node_id.node_id.namespace, depth, reference.node_class);
                                }
                                
                                // Early termination on exact match for speed
                                if exact_display_match || exact_browse_match {
                                    if verbose {
                                        println!("    🎯 Exact match found, stopping search early");
                                    }
                                    return Ok(found_nodes);
                                }
                            }
                            
                            // Add child nodes for next depth level
                            if depth < max_depth {
                                nodes_to_browse.push((reference.node_id.node_id.clone(), depth + 1));
                            }
                        }
                    }
                }
            }
        }
    }
    
    if verbose {
        println!("    📊 Depth-limited search completed: {} nodes searched, {} matches found", 
            browsed_nodes.len(), found_nodes.len());
    }
    
    Ok(found_nodes)
}

fn search_nodes_deep(
    session: &opcua::client::prelude::Session,
    search_term: &str,
    verbose: bool,
) -> Result<Vec<(NodeId, String, NodeClass)>> {
    let mut found_nodes = Vec::new();
    let mut nodes_to_browse = vec![
        NodeId::new(0, 85),   // Objects folder
        NodeId::new(0, 2253), // Server node
        NodeId::new(0, 86),   // Types folder
    ];
    let mut browsed_nodes = HashSet::new();
    let max_nodes = 500;  // Reduced from 2000 for better performance
    let max_queue = 200;  // Reduced queue size
    
    while let Some(node_id) = nodes_to_browse.pop() {
        let node_key = format!("{:?}", node_id);
        if browsed_nodes.contains(&node_key) || browsed_nodes.len() >= max_nodes {
            continue;
        }
        browsed_nodes.insert(node_key);
        
        if verbose && browsed_nodes.len() % 50 == 0 {
            println!("    🔍 Deep search: {} nodes (ns={})...", browsed_nodes.len(), node_id.namespace);
        }
        
        let browse_description = BrowseDescription {
            node_id: node_id.clone(),
            browse_direction: BrowseDirection::Forward,
            reference_type_id: ReferenceTypeId::HierarchicalReferences.into(),
            include_subtypes: true,
            node_class_mask: 0,
            result_mask: 0x3F,
        };
        
        // No delays - removed completely for speed
        if let Ok(Some(results)) = session.browse(&[browse_description]) {
            if let Some(result) = results.first() {
                if result.status_code.is_good() {
                    if let Some(ref references) = result.references {
                        for reference in references {
                            let display_name = reference.display_name.text.as_ref();
                            let browse_name = &reference.browse_name.name;
                            
                            let display_matches = display_name.to_lowercase().contains(&search_term.to_lowercase());
                            let browse_matches = browse_name.to_string().to_lowercase().contains(&search_term.to_lowercase());
                            
                            if display_matches || browse_matches {
                                found_nodes.push((
                                    reference.node_id.node_id.clone(),
                                    display_name.to_string(),
                                    reference.node_class,
                                ));
                                
                                if verbose {
                                    println!("    ✅ Found: '{}' (ns={}, class: {:?})", 
                                        display_name, reference.node_id.node_id.namespace, reference.node_class);
                                }
                            }
                            
                            // Add child nodes to queue with limits
                            if browsed_nodes.len() < max_nodes && nodes_to_browse.len() < max_queue {
                                nodes_to_browse.push(reference.node_id.node_id.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    
    if verbose {
        println!("    📊 Deep search completed: {} nodes searched, {} matches found", 
            browsed_nodes.len(), found_nodes.len());
    }
    
    Ok(found_nodes)
}

fn search_for_method(
    session: &opcua::client::prelude::Session,
    method_id: &str,
    verbose: bool,
) -> Result<(NodeId, NodeId)> {
    println!("\n{}", "🔍 Searching for method...".bright_blue());
    
    let found_methods = search_methods_by_name(session, method_id, verbose)?;
    
    if found_methods.is_empty() {
        // Provide comprehensive troubleshooting guidance
        let error_msg = format!(
            "No methods found with name: '{}'\n\n  💡 Troubleshooting steps:\n     1. Use 'browse' to explore the server and find available methods:\n        opcua-walker browse\n     2. Look for the method in different server areas:\n        opcua-walker browse --node \"ns=0;i=85\"  # Objects folder\n        opcua-walker browse --node \"ns=0;i=2253\" # Server node\n     3. Try searching with partial names or check for typos:\n        opcua-walker call \"reboot\" --verbose  # case-insensitive\n        opcua-walker call \"restart\" --verbose # alternative name\n     4. Use exact node IDs if you know them:\n        opcua-walker call \"ns=X;s=MethodName\" \"ns=X;s=ObjectName\"\n     5. Check if the method might be in a namespace other than 0:\n        opcua-walker browse --all-attributes\n     6. Enable verbose mode for detailed search diagnostics:\n        opcua-walker call \"{}\" --verbose\n\n  🔍 The progressive search looked in:\n     • Quick search: depth 3 from Objects, Server, Types folders\n     • Broader search: depth 5 from same areas  \n     • Deep search: up to 500 nodes across all namespaces\n\n  ⚠️  If the method exists but wasn't found, it might be:\n     • In a custom namespace (ns=1, ns=2, etc.)\n     • Deeper than 5 levels in the hierarchy\n     • Referenced through non-standard references\n     • Requires special permissions to access", 
            method_id, method_id
        );
        return Err(anyhow::anyhow!(error_msg));
    }
    
    if found_methods.len() > 1 {
        println!("✅ Found {} methods with name '{}':", found_methods.len(), method_id);
        for (i, (_method_node, object_node, object_name)) in found_methods.iter().enumerate() {
            println!("  [{}] {} on object: {} ({})", 
                i + 1,
                method_id.bright_white(),
                object_name.bright_cyan(),
                format!("{}", object_node).dimmed()
            );
        }
        println!("Using the first match...");
    }
    
    let (method_node, object_node, object_name) = &found_methods[0];
    println!("✅ Found method: {} on object: {}", 
        method_id.bright_green(), 
        object_name.bright_cyan()
    );
    println!("   Method Node: {}", format!("{}", method_node).dimmed());
    println!("   Object Node: {}", format!("{}", object_node).dimmed());
    
    Ok((method_node.clone(), object_node.clone()))
}

fn search_methods_by_name(
    session: &opcua::client::prelude::Session,
    method_name: &str,
    verbose: bool,
) -> Result<Vec<(NodeId, NodeId, String)>> {
    // Use progressive search strategy like read --search for better performance
    if verbose {
        println!("    🎯 Starting progressive method search for: '{}'", method_name);
    }
    
    // Stage 1: Quick method search (depth-limited)
    let quick_results = search_methods_with_depth_limit(session, method_name, 3, verbose)?;
    if !quick_results.is_empty() {
        if verbose {
            println!("    ✅ Quick method search found {} matches", quick_results.len());
        }
        return Ok(quick_results);
    }
    
    // Stage 2: Broader method search
    if verbose {
        println!("    🔄 No quick matches, trying broader method search...");
    }
    let broader_results = search_methods_with_depth_limit(session, method_name, 5, verbose)?;
    if !broader_results.is_empty() {
        if verbose {
            println!("    ✅ Broader method search found {} matches", broader_results.len());
        }
        return Ok(broader_results);
    }
    
    // Stage 3: Deep method search as last resort
    if verbose {
        println!("    🔍 No matches yet, trying deep method search...");
    }
    search_methods_with_limits(session, method_name, 500, 200, verbose)
}

fn search_methods_with_depth_limit(
    session: &opcua::client::prelude::Session,
    method_name: &str,
    max_depth: u32,
    verbose: bool,
) -> Result<Vec<(NodeId, NodeId, String)>> {
    let mut found_methods: Vec<(NodeId, NodeId, String)> = Vec::new();
    let mut nodes_to_browse = vec![
        (NodeId::new(0, 85), 0),   // Objects folder
        (NodeId::new(0, 2253), 0), // Server node
        (NodeId::new(0, 86), 0),   // Types folder
    ];
    let mut browsed_nodes = HashSet::new();
    
    while let Some((node_id, depth)) = nodes_to_browse.pop() {
        if depth > max_depth {
            continue;
        }
        
        let node_key = format!("{:?}", node_id);
        if browsed_nodes.contains(&node_key) {
            continue;
        }
        browsed_nodes.insert(node_key);
        
        let browse_description = BrowseDescription {
            node_id: node_id.clone(),
            browse_direction: BrowseDirection::Forward,
            reference_type_id: ReferenceTypeId::HierarchicalReferences.into(),
            include_subtypes: true,
            node_class_mask: 0,
            result_mask: 0x3F,
        };
        
        // No artificial delays for speed
        if let Ok(Some(results)) = session.browse(&[browse_description]) {
            if let Some(result) = results.first() {
                if result.status_code.is_good() {
                    if let Some(ref references) = result.references {
                        for reference in references {
                            let display_name = reference.display_name.text.as_ref();
                            let browse_name = &reference.browse_name.name;
                            
                            // Only process Method nodes
                            if reference.node_class == NodeClass::Method {
                                let exact_display_match = display_name.eq_ignore_ascii_case(method_name);
                                let exact_browse_match = browse_name.to_string().eq_ignore_ascii_case(method_name);
                                let partial_display_match = display_name.to_lowercase().contains(&method_name.to_lowercase());
                                let partial_browse_match = browse_name.to_string().to_lowercase().contains(&method_name.to_lowercase());
                                
                                if exact_display_match || exact_browse_match || partial_display_match || partial_browse_match {
                                    let parent_object_id = node_id.clone();
                                    let parent_name = get_node_display_name(session, &parent_object_id)
                                        .unwrap_or_else(|_| format!("{}", parent_object_id));
                                    
                                    found_methods.push((
                                        reference.node_id.node_id.clone(),
                                        parent_object_id,
                                        parent_name.clone(),
                                    ));
                                    
                                    if verbose {
                                        println!("    ✅ Found method: '{}' (ns={}, depth={}) in object: '{}'", 
                                            display_name, reference.node_id.node_id.namespace, depth, parent_name);
                                    }
                                    
                                    // Early termination on exact match
                                    if exact_display_match || exact_browse_match {
                                        if verbose {
                                            println!("    🎯 Exact method match found, stopping search early");
                                        }
                                        return Ok(found_methods);
                                    }
                                }
                            }
                            
                            // Add child nodes for next depth level
                            if depth < max_depth {
                                nodes_to_browse.push((reference.node_id.node_id.clone(), depth + 1));
                            }
                        }
                    }
                }
            }
        }
    }
    
    if verbose {
        println!("    📊 Depth-limited method search completed: {} nodes searched, {} methods found", 
            browsed_nodes.len(), found_methods.len());
    }
    
    Ok(found_methods)
}

fn search_methods_with_limits(
    session: &opcua::client::prelude::Session,
    method_name: &str,
    max_nodes: usize,
    max_queue: usize,
    verbose: bool,
) -> Result<Vec<(NodeId, NodeId, String)>> {
    let mut found_methods: Vec<(NodeId, NodeId, String)> = Vec::new();
    let mut nodes_to_browse = vec![
        NodeId::new(0, 85),   // Objects folder
        NodeId::new(0, 2253), // Server node
        NodeId::new(0, 86),   // Types folder
    ];
    let mut browsed_nodes = HashSet::new();
    
    while let Some(node_id) = nodes_to_browse.pop() {
        let node_key = format!("{:?}", node_id);
        if browsed_nodes.contains(&node_key) || browsed_nodes.len() >= max_nodes {
            continue;
        }
        browsed_nodes.insert(node_key);
        
        if verbose && browsed_nodes.len() % 50 == 0 {
            println!("    🔍 Searched {} nodes (ns={})...", browsed_nodes.len(), node_id.namespace);
        }
        
        // Browse for all child nodes (following the read --search pattern)
        let browse_description = BrowseDescription {
            node_id: node_id.clone(),
            browse_direction: BrowseDirection::Forward,
            reference_type_id: ReferenceTypeId::HierarchicalReferences.into(),
            include_subtypes: true,
            node_class_mask: 0, // All node classes
            result_mask: 0x3F, // All attributes
        };
        
        // No delays - removed completely for speed
        if let Ok(Some(results)) = session.browse(&[browse_description]) {
            if let Some(result) = results.first() {
                if result.status_code.is_good() {
                    if let Some(ref references) = result.references {
                        for reference in references {
                            let display_name = reference.display_name.text.as_ref();
                            let browse_name = &reference.browse_name.name;
                            
                            // Check if this is a method and matches our search
                            if reference.node_class == NodeClass::Method {
                                let exact_display_match = display_name.eq_ignore_ascii_case(method_name);
                                let exact_browse_match = browse_name.to_string().eq_ignore_ascii_case(method_name);
                                let partial_display_match = display_name.to_lowercase().contains(&method_name.to_lowercase());
                                let partial_browse_match = browse_name.to_string().to_lowercase().contains(&method_name.to_lowercase());
                                
                                if exact_display_match || exact_browse_match || partial_display_match || partial_browse_match {
                                    let parent_object_id = node_id.clone();
                                    let parent_name = get_node_display_name(session, &parent_object_id)
                                        .unwrap_or_else(|_| format!("{}", parent_object_id));
                                    
                                    found_methods.push((
                                        reference.node_id.node_id.clone(),
                                        parent_object_id,
                                        parent_name.clone(),
                                    ));
                                    
                                    if verbose {
                                        println!("    ✅ Found method: '{}' (ns={}) in object: '{}'", 
                                            display_name, reference.node_id.node_id.namespace, parent_name);
                                    }
                                    
                                    // Return immediately on exact match for efficiency
                                    if exact_display_match || exact_browse_match {
                                        if verbose {
                                            println!("    🎯 Exact match found, stopping search");
                                        }
                                        return Ok(found_methods);
                                    }
                                }
                            }
                            
                            // Add ALL child nodes to search queue (like read --search does)
                            // This allows discovery of nodes in other namespaces
                            if browsed_nodes.len() < max_nodes && nodes_to_browse.len() < max_queue {
                                nodes_to_browse.push(reference.node_id.node_id.clone());
                            }
                        }
                    }
                }
            }
        }
    }
    
    if verbose {
        println!("    📊 Search completed: {} nodes searched, {} methods found", 
            browsed_nodes.len(), found_methods.len());
    }
    
    Ok(found_methods)
}





fn get_node_display_name(
    session: &opcua::client::prelude::Session,
    node_id: &NodeId,
) -> Result<String> {
    let read_request = vec![ReadValueId {
        node_id: node_id.clone(),
        attribute_id: AttributeId::DisplayName as u32,
        index_range: UAString::null(),
        data_encoding: QualifiedName::null(),
    }];
    
    match session.read(&read_request, TimestampsToReturn::Neither, 0.0) {
        Ok(results) => {
            if let Some(result) = results.first() {
                if let Some(status) = &result.status {
                    if status.is_good() {
                        if let Some(value) = &result.value {
                            if let Variant::LocalizedText(text) = value {
                                return Ok(text.text.to_string());
                            }
                        }
                    }
                }
            }
            Ok(format!("{}", node_id))
        }
        Err(_) => Ok(format!("{}", node_id)),
    }
}
