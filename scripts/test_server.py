#!/usr/bin/env python3
"""
OPC-UA Test Server for testing the opcua-walker CLI tool.
Creates various test variables with dynamic values.
"""

import asyncio
import logging
import signal
import sys
from datetime import datetime
import math

from asyncua import Server
from asyncua.common.node import Node

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

class OPCUATestServer:
    def __init__(self):
        self.server = None
        self.running = False
        
    async def init_server(self):
        """Initialize the OPC-UA server with test variables."""
        self.server = Server()
        await self.server.init()
        
        # Set server endpoint
        self.server.set_endpoint('opc.tcp://0.0.0.0:4840/opcua/')
        
        # Set server properties
        self.server.set_server_name("OPC-UA Test Server for opcua-walker")
        
        # Register namespace
        uri = 'http://test-opcua-server.local'
        idx = await self.server.register_namespace(uri)
        
        # Get objects node
        objects = self.server.get_objects_node()
        
        # Create test folder
        test_folder = await objects.add_folder(idx, 'TestVariables')
        
        # Create various test variables
        self.counter_var = await test_folder.add_variable(idx, 'Counter', 0)
        self.temperature_var = await test_folder.add_variable(idx, 'Temperature', 23.5)
        self.pressure_var = await test_folder.add_variable(idx, 'Pressure', 1013.25)
        self.status_var = await test_folder.add_variable(idx, 'Status', 'Running')
        self.timestamp_var = await test_folder.add_variable(idx, 'Timestamp', datetime.now().isoformat())
        self.boolean_var = await test_folder.add_variable(idx, 'Boolean', True)
        self.string_var = await test_folder.add_variable(idx, 'DynamicString', 'Hello OPC-UA Walker!')
        
        # Make variables writable
        await self.counter_var.set_writable()
        await self.temperature_var.set_writable()
        await self.pressure_var.set_writable()
        await self.status_var.set_writable()
        await self.timestamp_var.set_writable()
        await self.boolean_var.set_writable()
        await self.string_var.set_writable()
        
        # Create test methods for testing method calling functionality
        server_object = await objects.add_object(idx, 'ServerObject')
        
        # Add Reboot method
        reboot_method = await server_object.add_method(
            idx, 
            'Reboot', 
            self.reboot_method,
            [], # input arguments
            [] # output arguments  
        )
        
        # Add AddNumbers method with input arguments
        from asyncua import ua
        add_numbers_method = await server_object.add_method(
            idx,
            'AddNumbers',
            self.add_numbers_method,
            [ua.VariantType.Int32, ua.VariantType.Int32], # two integer inputs
            [ua.VariantType.Int32] # one integer output
        )
        
        # Add ResetCounter method 
        reset_counter_method = await server_object.add_method(
            idx,
            'ResetCounter', 
            self.reset_counter_method,
            [], # no input arguments
            [ua.VariantType.String] # string output
        )
        
        logger.info("🏃 OPC-UA Test Server initialized")
        logger.info("📡 Endpoint: opc.tcp://localhost:4840/opcua/")
        logger.info("📊 Test variables created in namespace ns=2:")
        logger.info("  • ns=2;s=Counter (Int32) - Incrementing counter")
        logger.info("  • ns=2;s=Temperature (Float) - Simulated temperature") 
        logger.info("  • ns=2;s=Pressure (Float) - Simulated pressure")
        logger.info("  • ns=2;s=Status (String) - Server status")
        logger.info("  • ns=2;s=Timestamp (String) - Current timestamp")
        logger.info("  • ns=2;s=Boolean (Boolean) - Alternating boolean")
        logger.info("  • ns=2;s=DynamicString (String) - Dynamic string")
        logger.info("📞 Test methods created:")
        logger.info("  • Reboot - Restart server (no arguments)")
        logger.info("  • AddNumbers - Add two integers")
        logger.info("  • ResetCounter - Reset counter to zero")
        logger.info("")
        logger.info("💡 Test with CLI:")
        logger.info("   ./target/release/opcua-walker info")
        logger.info("   ./target/release/opcua-walker browse")
        logger.info("   ./target/release/opcua-walker read \"ns=2;s=Counter\"")
        logger.info("   ./target/release/opcua-walker call \"Reboot\"")
        logger.info("   ./target/release/opcua-walker call \"AddNumbers\" --args \"5,10\"")
        logger.info("⏹️  Press Ctrl+C to stop server")
        
    async def reboot_method(self, parent):
        """Simulated reboot method."""
        logger.info("🔄 Reboot method called - simulating server restart...")
        await self.status_var.write_value("Rebooting")
        await asyncio.sleep(1)  # Simulate restart delay
        await self.status_var.write_value("Running")
        return []
        
    async def add_numbers_method(self, parent, a, b):
        """Add two numbers and return the result."""
        result = a + b
        logger.info(f"➕ AddNumbers method called: {a} + {b} = {result}")
        return [result]
        
    async def reset_counter_method(self, parent):
        """Reset the counter variable to zero."""
        logger.info("🔄 ResetCounter method called")
        await self.counter_var.write_value(0)
        return ["Counter reset to 0"]
        
    async def update_variables(self):
        """Update test variables with dynamic values."""
        counter = 0
        
        while self.running:
            try:
                counter += 1
                current_time = datetime.now()
                
                # Update counter
                await self.counter_var.write_value(counter)
                
                # Update temperature with sine wave (20°C ± 10°C)
                temp = 20.0 + 10.0 * math.sin(counter * 0.1)
                await self.temperature_var.write_value(round(temp, 2))
                
                # Update pressure with random walk
                base_pressure = 1013.25
                variation = 50 * math.sin(counter * 0.05) + 25 * math.cos(counter * 0.07)
                pressure = base_pressure + variation
                await self.pressure_var.write_value(round(pressure, 2))
                
                # Update status
                statuses = ['Running', 'Active', 'Ready', 'Online']
                status = statuses[counter % len(statuses)]
                await self.status_var.write_value(status)
                
                # Update timestamp
                await self.timestamp_var.write_value(current_time.isoformat())
                
                # Update boolean (alternates every 5 seconds)
                await self.boolean_var.write_value((counter // 3) % 2 == 0)
                
                # Update dynamic string
                await self.string_var.write_value(f'Message #{counter} - {current_time.strftime("%H:%M:%S")}')
                
                # Log every 10 updates
                if counter % 10 == 0:
                    logger.info(f"📈 Updated variables (cycle #{counter})")
                
                await asyncio.sleep(2)  # Update every 2 seconds
                
            except Exception as e:
                logger.error(f"Error updating variables: {e}")
                await asyncio.sleep(1)
    
    async def start_server(self):
        """Start the OPC-UA server."""
        self.running = True
        
        async with self.server:
            logger.info("🚀 OPC-UA Test Server started successfully")
            
            # Start variable update task
            update_task = asyncio.create_task(self.update_variables())
            
            try:
                # Keep server running
                await update_task
            except asyncio.CancelledError:
                logger.info("📡 Server update task cancelled")
            except Exception as e:
                logger.error(f"Server error: {e}")
                
    def stop_server(self):
        """Stop the server gracefully."""
        logger.info("🛑 Stopping OPC-UA Test Server...")
        self.running = False

async def main():
    """Main function."""
    server = OPCUATestServer()
    
    # Setup signal handlers for graceful shutdown
    def signal_handler():
        logger.info("🔔 Received shutdown signal")
        server.stop_server()
    
    # Register signal handlers
    loop = asyncio.get_event_loop()
    for sig in (signal.SIGTERM, signal.SIGINT):
        loop.add_signal_handler(sig, signal_handler)
    
    try:
        await server.init_server()
        await server.start_server()
    except KeyboardInterrupt:
        logger.info("🔔 Received KeyboardInterrupt")
    except Exception as e:
        logger.error(f"❌ Server failed: {e}")
    finally:
        logger.info("👋 OPC-UA Test Server stopped")

if __name__ == '__main__':
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n👋 Goodbye!")
        sys.exit(0) 
