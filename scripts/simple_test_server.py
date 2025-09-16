#!/usr/bin/env python3
"""
Extremely minimal OPC-UA test server using different library.
"""

from opcua import Server
import time

if __name__ == "__main__":
    # Create server
    server = Server()
    server.set_endpoint("opc.tcp://0.0.0.0:4840/opcua/")
    
    # Setup namespace
    uri = "http://minimal.test"
    idx = server.register_namespace(uri)
    
    # Get Objects node
    objects = server.get_objects_node()
    
    # Add a simple object with a method
    myobj = objects.add_object(idx, "MyObject")
    
    # Add a simple method
    def reboot(parent):
        print("Reboot method called!")
        return []
    
    myobj.add_method(idx, "Reboot", reboot, [], [])
    
    # Add a simple variable
    myvar = myobj.add_variable(idx, "Counter", 0)
    myvar.set_writable()
    
    # Start server
    server.start()
    print("Server started at opc.tcp://localhost:4840/opcua/")
    print("Press Ctrl+C to stop")
    
    try:
        while True:
            time.sleep(1)
    except KeyboardInterrupt:
        pass
    finally:
        server.stop()
        print("Server stopped")