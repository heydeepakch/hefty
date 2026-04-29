"""Tiny static dev server for the Tauri UI.

Resolves the UI directory relative to this script's location, so it works
regardless of where Tauri spawns it from.
"""

import os
import sys
import http.server
import socketserver

PORT = 1420
HOST = "127.0.0.1"

UI_DIR = os.path.normpath(
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "app", "ui")
)

sys.stderr.write(f"dev-server: launched from cwd={os.getcwd()}\n")
sys.stderr.write(f"dev-server: script at {os.path.abspath(__file__)}\n")

if not os.path.isdir(UI_DIR):
    sys.stderr.write(f"dev-server: UI directory not found at {UI_DIR}\n")
    sys.exit(1)

os.chdir(UI_DIR)
sys.stderr.write(f"dev-server: serving {UI_DIR} at http://{HOST}:{PORT}\n")
sys.stderr.flush()

handler = http.server.SimpleHTTPRequestHandler
with socketserver.TCPServer((HOST, PORT), handler) as httpd:
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        pass
