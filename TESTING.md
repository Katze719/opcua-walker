# Testing OPC-UA Walker

This guide provides various options for testing the OPC-UA Walker CLI tool with real OPC-UA servers.

## Quick Testing Options

### Option 1: Docker-based Test Server (Recommended)

The easiest way to test is using our custom Docker-based OPC-UA server:

```bash
# Start the Python-based OPC-UA test server
docker-compose up -d

# Test with the CLI
./target/release/opcua-walker info
./target/release/opcua-walker discover  
./target/release/opcua-walker browse
./target/release/opcua-walker read "ns=2;s=Counter"

# Stop the server
docker-compose down
```
