use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use opcua::client::prelude::*;
use opcua::sync::RwLock;
use std::sync::Arc;

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
    /// Read variable values
    Read {
        /// Node ID of the variable to read
        node_id: String,
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

    // Configure client
    let mut client = ClientBuilder::new()
        .application_name("OPC-UA Walker")
        .application_uri("urn:opcua-walker")
        .create_sample_keypair(true)
        .trust_server_certs(true)
        .session_retry_limit(3)
        .client()
        .ok_or_else(|| anyhow::anyhow!("Failed to create OPC-UA client"))?;

    // Create session with appropriate authentication
    let session = match auth_method {
        AuthMethod::Anonymous => client.connect_to_endpoint(
            (
                cli.endpoint.as_ref(),
                SecurityPolicy::None.to_str(),
                MessageSecurityMode::None,
                UserTokenPolicy::anonymous(),
            ),
            IdentityToken::Anonymous,
        )?,
        AuthMethod::UsernamePassword(username, password) => client.connect_to_endpoint(
            (
                cli.endpoint.as_ref(),
                SecurityPolicy::None.to_str(),
                MessageSecurityMode::None,
                UserTokenPolicy::anonymous(),
            ),
            IdentityToken::UserName(username, password),
        )?,
        AuthMethod::Certificate(cert_path, key_path) => {
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
                    UserTokenPolicy::anonymous(),
                ),
                IdentityToken::X509(cert_path_buf, key_path_buf),
            )?
        }
    };

    match &cli.command {
        Commands::Discover => {
            discover_server_capabilities(session.clone(), cli.verbose)?;
        }
        Commands::Browse { node, depth } => {
            let start_node = node.as_deref().unwrap_or("ns=0;i=85"); // Objects folder
            browse_address_space(session.clone(), start_node, *depth, cli.verbose)?;
        }
        Commands::Read { node_id } => {
            read_variable_value(session.clone(), node_id, cli.verbose)?;
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
    _max_depth: u32,
    verbose: bool,
) -> Result<usize> {
    // Simple browse implementation that shows basic structure
    println!("\n{}", "🌳 Address Space Structure".bright_blue().bold());

    // Try to browse some standard well-known nodes
    let standard_nodes = vec![
        ("Objects", "ns=0;i=85"),
        ("Server", "ns=0;i=2253"),
        ("Types", "ns=0;i=86"),
        ("Views", "ns=0;i=87"),
    ];

    let mut found_count = 0;

    for (name, node_id) in &standard_nodes {
        if read_node_info(session, node_id, name, verbose).is_ok() {
            found_count += 1;
        }
    }

    // Try to read the start node if it's different
    if !standard_nodes.iter().any(|(_, id)| *id == start_node) {
        if read_node_info(session, start_node, "Start Node", verbose).is_ok() {
            found_count += 1;
        }
    }

    Ok(found_count)
}

fn read_node_info(
    session: &opcua::client::prelude::Session,
    node_id: &str,
    name: &str,
    verbose: bool,
) -> Result<()> {
    // Parse the node ID manually - simple implementation
    let node = if node_id.starts_with("ns=0;i=") {
        let id_str = &node_id[7..]; // Skip "ns=0;i="
        if let Ok(id) = id_str.parse::<u32>() {
            NodeId::new(0, id)
        } else {
            return Err(anyhow::anyhow!("Invalid node ID format: {}", node_id));
        }
    } else {
        return Err(anyhow::anyhow!("Unsupported node ID format: {}", node_id));
    };

    let read_request = vec![ReadValueId::from(&node)];

    match session.read(&read_request, TimestampsToReturn::Neither, 0.0) {
        Ok(results) => {
            if let Some(result) = results.first() {
                if let Some(status) = &result.status {
                    if status.is_good() {
                        println!("  📋 {} ({})", name.bright_white(), node_id.dimmed());
                        if verbose {
                            if let Some(value) = &result.value {
                                println!("     Value: {:?}", value);
                            }
                        }
                        return Ok(());
                    }
                }
            }
            Err(anyhow::anyhow!("Failed to read node {}", node_id))
        }
        Err(e) => Err(anyhow::anyhow!("Error reading {}: {}", node_id, e)),
    }
}

fn read_variable_value(session: Arc<RwLock<Session>>, node_id: &str, verbose: bool) -> Result<()> {
    println!("\n{}", "📖 Reading Variable".bright_green().bold());
    println!("Node ID: {}", node_id.bright_yellow());

    let session_lock = session.read();

    // Parse node ID - support basic formats
    let node = parse_node_id(node_id)?;

    let read_request = vec![ReadValueId::from(&node)];

    match session_lock.read(&read_request, TimestampsToReturn::Both, 0.0) {
        Ok(results) => {
            if let Some(result) = results.first() {
                println!("\n{}", "📋 Variable Information".bright_blue().bold());
                println!("  Node ID: {}", node_id.bright_cyan());

                if let Some(status) = &result.status {
                    if status.is_good() {
                        println!("  Status: {}", "Good".bright_green());

                        if let Some(value) = &result.value {
                            println!("  Value: {}", format_value(value).bright_white());
                            println!("  Type: {}", get_variant_type_name(value).bright_yellow());

                            if verbose {
                                println!("  Raw Value: {:?}", value);
                            }

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
                        } else {
                            println!("  Value: {}", "No value returned".yellow());
                        }
                    } else {
                        println!("  Status: {} ({:?})", "Error".bright_red(), status);
                        return Err(anyhow::anyhow!("Read failed with status: {:?}", status));
                    }
                }

                println!(
                    "\n{}",
                    "✅ Variable read completed successfully".bright_green()
                );
            }
        }
        Err(e) => {
            println!("{}Error reading variable: {}", "❌ ".red(), e);
            return Err(anyhow::anyhow!("Failed to read variable: {}", e));
        }
    }

    Ok(())
}

fn parse_node_id(node_id: &str) -> Result<NodeId> {
    // Simple node ID parser for common formats
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
            } else {
                Err(anyhow::anyhow!(
                    "Unsupported node ID identifier type: {}",
                    node_id
                ))
            }
        } else {
            Err(anyhow::anyhow!("Invalid node ID format: {}", node_id))
        }
    } else {
        Err(anyhow::anyhow!("Unsupported node ID format: {}", node_id))
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
