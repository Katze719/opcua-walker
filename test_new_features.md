# Testing New OPC-UA Walker Features

This document demonstrates the newly implemented features for method calling and search-by-name functionality.

## New Features Implemented

### 1. Method Calling (`call` command)

The new `call` command allows triggering methods on OPC-UA servers.

**Syntax:**
```bash
opcua-walker call <method-id> <object-id> [--args "arguments"] [--verbose]
```

**Examples:**
```bash
# Call a simple method without arguments
opcua-walker call "ns=2;s=ResetCounter" "ns=2;s=CounterObject"

# Call a method with simple arguments
opcua-walker call "ns=2;s=AddNumbers" "ns=2;s=MathObject" --args "5,10"

# Call a method with JSON arguments
opcua-walker call "ns=2;s=ProcessData" "ns=2;s=DataObject" --args '[{"value": 42, "name": "test"}]'

# Call with verbose output
opcua-walker call "ns=2;s=GetStatus" "ns=2;s=SystemObject" --verbose
```

**Features:**
- Supports multiple argument formats (comma-separated, JSON)
- Automatic type detection (boolean, integer, float, string)
- Displays method execution status and output arguments
- Detailed error reporting

### 2. Search by Name (`read --search` option)

The enhanced `read` command now supports searching for nodes by display name instead of requiring exact node IDs.

**Syntax:**
```bash
opcua-walker read --search "search-term1" "search-term2" [options]
```

**Examples:**
```bash
# Search for nodes containing "Counter" in their name
opcua-walker read --search "Counter"

# Search for multiple terms
opcua-walker read --search "Temperature" "Pressure" "Status"

# Search with all attributes
opcua-walker read --search "Counter" --all-attributes

# Search and include values for all nodes
opcua-walker read --search "Temperature" --include-value
```

**Features:**
- Case-insensitive partial matching on display names and browse names
- Searches through the entire address space recursively
- Displays all matching nodes before reading their information
- Supports all existing read options (--all-attributes, --include-value)
- Shows node class icons and types for easy identification

## Command Line Help

### Main Help
```bash
$ opcua-walker --help
A CLI tool for exploring OPC-UA servers and their capabilities

Usage: opcua-walker [OPTIONS] <COMMAND>

Commands:
  discover  Discover server capabilities and available services
  browse    Browse address space and show all available nodes
  read      Read node information and attributes
  call      Call a method on the server
  info      Show server information
  help      Print this message or the help of the given subcommand(s)

Options:
  -e, --endpoint <ENDPOINT>  OPC-UA Server Endpoint URL [default: opc.tcp://localhost:4840]
  -u, --username <USERNAME>  Username for authentication
  -p, --password <PASSWORD>  Password for authentication
  -c, --cert <CERT>          Client certificate file path for X.509 authentication
  -k, --key <KEY>            Client private key file path for X.509 authentication
  -v, --verbose              Enable detailed output
  -h, --help                 Print help
  -V, --version              Print version
```

### Read Command Help
```bash
$ opcua-walker read --help
Read node information and attributes

Usage: opcua-walker read [OPTIONS] [NODE_IDS]...

Arguments:
  [NODE_IDS]...  Node ID(s) to read (can specify multiple) or name to search for

Options:
  -a, --all-attributes  Read all available attributes (default: basic info only)
  -V, --include-value   Force include node value for all nodes (Variable nodes include values by default)
  -s, --search          Search for nodes by display name instead of using exact node ID
  -h, --help            Print help
```

### Call Command Help
```bash
$ opcua-walker call --help
Call a method on the server

Usage: opcua-walker call [OPTIONS] <METHOD_ID> <OBJECT_ID>

Arguments:
  <METHOD_ID>  Method node ID to call
  <OBJECT_ID>  Object node ID that owns the method

Options:
  -a, --args <ARGS>  Input arguments for the method (JSON format or simple values)
  -v, --verbose      Show detailed call information
  -h, --help         Print help
```

## Implementation Details

### Method Calling Implementation
- Uses OPC-UA Call service through the opcua crate
- Supports multiple argument formats with automatic type detection
- Handles method results and displays output arguments
- Provides detailed error reporting for failed method calls

### Search Implementation
- Recursively browses the address space starting from Objects, Server, and Types nodes
- Compares both display names and browse names against search terms
- Implements safety limits to prevent infinite loops (max 1000 nodes searched)
- Returns structured results with node IDs, names, and classes
- Integrates seamlessly with existing read functionality

### Error Handling
- Comprehensive error messages for connection issues
- Graceful handling of method call failures
- Search timeout protection and result limiting
- Type conversion error handling for arguments

## Benefits

These new features address the original German requirements:

1. **"mein opcua tool methods triggern kann auf dem server"** ✅
   - The `call` command enables triggering methods on OPC-UA servers
   - Supports various argument types and formats
   - Provides clear feedback on method execution results

2. **"einen read auf einen string machen kann wo dann die node gesucht wird mit dem passenden namen"** ✅
   - The `--search` option allows reading nodes by searching for their names
   - No need to know exact namespace and node ID
   - Supports partial matching for easier discovery

The implementation maintains backward compatibility while adding powerful new functionality for OPC-UA server interaction.