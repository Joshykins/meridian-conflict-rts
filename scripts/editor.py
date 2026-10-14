#!/usr/bin/env python3
"""Send a JSON request to a running Meridian map editor.

Usage: editor.py [--mailbox DIR] JSON_OR_FILE
       editor.py [--mailbox DIR] --mcp
Each edit request is one undo batch. See docs/EDITOR.md for request schemas.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import uuid

def mailbox(explicit):
    if explicit:
        return Path(explicit)
    if os.name == "nt":
        root = Path(os.environ["APPDATA"])
    elif "microsoft" in os.uname().release.lower():
        windows = subprocess.check_output(["powershell.exe", "-NoProfile", "-Command", "$env:APPDATA"], text=True).strip()
        root = Path(subprocess.check_output(["wslpath", windows], text=True).strip())
    else:
        root = Path(os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config")))
    choices = [p.parent for p in (root / "meridian-conflict" / "editor").glob("*/alive") if time.time()-p.stat().st_mtime < 5]
    if len(choices) != 1:
        raise RuntimeError(f"Found {len(choices)} active editors. Open one, or choose --mailbox DIR.")
    return choices[0]

def send(box, request):
    payload = json.dumps(request).encode()
    if len(payload) > 262144:
        raise ValueError("Request exceeds 256 KiB")
    token = uuid.uuid4().hex
    pending = box / (token + ".tmp")
    pending.write_bytes(payload)
    pending.rename(box / (token + ".req.json"))
    answer = box / (token + ".reply.json")
    deadline = time.monotonic() + 120
    while time.monotonic() < deadline:
        if answer.exists():
            reply = json.loads(answer.read_text())
            answer.unlink()
            return reply
        time.sleep(0.1)
    raise TimeoutError("Editor did not answer within 120 seconds. The outcome is unknown; inspect state before retrying.")

def mcp(box):
    for line in sys.stdin:
        message = {}
        try:
            message = json.loads(line)
            if "id" not in message:
                continue
            method = message.get("method")
            if method == "initialize":
                result = {"protocolVersion": "2024-11-05", "capabilities": {"tools": {}}, "serverInfo": {"name": "meridian-editor", "version": "1"}}
            elif method == "tools/list":
                result = {"tools": [{"name": "editor", "description": "Edit and inspect the open Meridian map editor. Operations: state, tool, gizmo, capture_camera, recall_camera, edit, undo, redo, save, camera, shot, test, stop, debug, reload, model_calls, catalogue, choose_unit, choose_doodad, select, close.",
                    "inputSchema": {"type": "object", "properties": {"request": {"type": "object"}}, "required": ["request"]}}]}
            elif method == "tools/call":
                reply = send(box, message["params"]["arguments"]["request"])
                result = {"content": [{"type": "text", "text": json.dumps(reply)}], "isError": not reply.get("ok", False)}
            elif method == "ping":
                result = {}
            else:
                raise ValueError("Unsupported method")
            print(json.dumps({"jsonrpc": "2.0", "id": message["id"], "result": result}), flush=True)
        except Exception as error:
            print(json.dumps({"jsonrpc": "2.0", "id": message.get("id"), "error": {"code": -32603, "message": str(error)}}), flush=True)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mailbox")
    parser.add_argument("--mcp", action="store_true")
    parser.add_argument("request", nargs="?")
    args = parser.parse_args()
    box = mailbox(args.mailbox)
    if args.mcp:
        return mcp(box)
    if not args.request:
        parser.error("Give a JSON request or file")
    text = args.request if args.request.lstrip().startswith("{") else Path(args.request).read_text()
    reply = send(box, json.loads(text))
    print(json.dumps(reply, indent=2))
    return 0 if reply.get("ok") else 1

if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
