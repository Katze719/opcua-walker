use anyhow::Result;
use clap::Parser;
use tracing::debug;

mod client;
mod commands;
mod types;
mod utils;

use crate::client::OpcUaClient;
use crate::commands::Commands;
use crate::types::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize tracing
    init_tracing(cli.verbose);
    
    debug!("Starting OPC-UA Walker v{}", env!("CARGO_PKG_VERSION"));
    
    // Create and configure the OPC-UA client
    let mut client = OpcUaClient::new(&cli).await?;
    
    // Connect to the server
    client.connect().await?;
    
    // Execute the requested command
    let result = match &cli.command {
        Commands::Discover => commands::discover::execute(&mut client).await,
        Commands::Browse { node, depth, compact, values } => {
            commands::browse::execute(&mut client, node.as_deref(), *depth, *compact, *values).await
        }
        Commands::Read { node_ids, all_attributes, include_value, search } => {
            commands::read::execute(
                &mut client, 
                node_ids, 
                *all_attributes, 
                *include_value, 
                *search
            ).await
        }
        Commands::Call { method_id, object_id, args, verbose } => {
            commands::call::execute(
                &mut client, 
                method_id, 
                object_id.as_deref(), 
                args.as_deref(),
                *verbose
            ).await
        }
        Commands::Info => commands::info::execute(&mut client).await,
    };
    
    // Disconnect gracefully
    client.disconnect().await?;
    
    result
}

fn init_tracing(verbose: bool) {
    let filter = if verbose {
        "opcua_walker=debug,opcua_async=info"
    } else {
        "opcua_walker=info,opcua_async=warn"
    };
    
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_level(verbose)
        .init();
}