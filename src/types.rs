use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "opcua-walker")]
#[command(about = "A modern async CLI tool for exploring OPC-UA servers")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    /// OPC-UA Server Endpoint URL
    #[arg(short, long, default_value = "opc.tcp://localhost:4840")]
    pub endpoint: String,

    /// Username for authentication
    #[arg(short, long)]
    pub username: Option<String>,

    /// Password for authentication
    #[arg(short, long)]
    pub password: Option<String>,

    /// Client certificate file path for X.509 authentication
    #[arg(short, long)]
    pub cert: Option<String>,

    /// Client private key file path for X.509 authentication  
    #[arg(short, long)]
    pub key: Option<String>,

    /// Enable detailed output and debug logging
    #[arg(short, long)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
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

        /// Use compact view for output (less verbose table)
        #[arg(short, long)]
        compact: bool,

        /// Read and display values for all Variable nodes
        #[arg(short = 'V', long)]
        values: bool,
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
    
    /// Show server information and connection details
    Info,
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub username: Option<String>,
    pub password: Option<String>,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
}

impl From<&Cli> for AuthConfig {
    fn from(cli: &Cli) -> Self {
        Self {
            username: cli.username.clone(),
            password: cli.password.clone(),
            cert_path: cli.cert.clone(),
            key_path: cli.key.clone(),
        }
    }
}