#!/usr/bin/env python3
"""
Minimal OPC-UA Test Server for testing the opcua-walker CLI tool.
Uses minimal configuration to avoid encoding limits issues.
"""

import asyncio
import logging
import signal
import sys
from datetime import datetime

from asyncua import Server
from asyncua import ua

logging.basicConfig(level=logging.WARNING)  # Reduced logging
logger = logging.getLogger(__name__)

class MinimalOPCUATestServer:
    def __init__(self):
        self.server = None
        self.running = False
        
    async def init_server(self):
        """Initialize the OPC-UA server with minimal configuration."""
        self.server = Server()
        await self.server.init()
        
        # Set server endpoint
        self.server.set_endpoint('opc.tcp://0.0.0.0:4840/opcua/')
        
        # Set minimal server properties
        self.server.set_server_name("Minimal OPC-UA Test Server")
        
        # Register namespace
        uri = 'http://minimal-test-server.local'
        idx = await self.server.register_namespace(uri)
        
        # Get objects node
        objects = self.server.get_objects_node()
        
        # Create a simple server object with methods
        server_object = await objects.add_object(idx, 'ServerObject')
        
        # Add Reboot method (no arguments)
        await server_object.add_method(
            idx, 
            'Reboot', 
            self.reboot_method,
            [], # input arguments
            [] # output arguments  
        )
        
        # Add AddNumbers method with arguments
        await server_object.add_method(
            idx,
            'AddNumbers',
            self.add_numbers_method,
            [ua.VariantType.Int32, ua.VariantType.Int32], # two integer inputs
            [ua.VariantType.Int32] # one integer output
        )
        
        # Add simple test variable
        self.counter_var = await server_object.add_variable(idx, 'Counter', 0)
        await self.counter_var.set_writable()
        
        logger.warning("🏃 Minimal OPC-UA Test Server initialized")
        logger.warning("📡 Endpoint: opc.tcp://localhost:4840/opcua/")
        logger.warning("📞 Methods: Reboot, AddNumbers")
        logger.warning("📊 Variables: Counter")
        logger.warning("⏹️  Press Ctrl+C to stop server")
        
    async def reboot_method(self, parent):
        """Simulated reboot method."""
        logger.warning("🔄 Reboot method called")
        return []
        
    async def add_numbers_method(self, parent, a, b):
        """Add two numbers and return the result."""
        result = a + b
        logger.warning(f"➕ AddNumbers method called: {a} + {b} = {result}")
        return [result]
        
    async def start_server(self):
        """Start the OPC-UA server."""
        self.running = True
        
        async with self.server:
            logger.warning("🚀 Minimal OPC-UA Test Server started successfully")
            
            try:
                # Keep server running
                while self.running:
                    await asyncio.sleep(1)
            except asyncio.CancelledError:
                logger.warning("📡 Server cancelled")
            except Exception as e:
                logger.error(f"Server error: {e}")
                
    def stop_server(self):
        """Stop the server gracefully."""
        logger.warning("🛑 Stopping Minimal OPC-UA Test Server...")
        self.running = False

async def main():
    """Main server function."""
    server = MinimalOPCUATestServer()
    
    # Set up graceful shutdown
    def signal_handler(signum, frame):
        print("\n🛑 Received shutdown signal...")
        server.stop_server()
        
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    
    try:
        await server.init_server()
        await server.start_server()
    except KeyboardInterrupt:
        print("\n🛑 Keyboard interrupt received")
    except Exception as e:
        print(f"❌ Server error: {e}")
    finally:
        server.stop_server()
        print("👋 Server stopped")

if __name__ == "__main__":
    asyncio.run(main())