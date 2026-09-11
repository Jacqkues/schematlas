#!/usr/bin/env python3
"""Deterministic ACP fixture, not an AI model. Use Python as the executable.
Arguments: ["/absolute/path/to/mock-acp-agent.py"]
Prompts containing 'permission' exercise permission cards; 'wait' exercises cancel.
"""
import json
import sys

pending = None
permission_prompt = None

def send(value):
    print(json.dumps({"jsonrpc": "2.0", **value}), flush=True)

def reply(identifier, result):
    send({"id": identifier, "result": result})

def text(value):
    send({"method": "session/update", "params": {"sessionId": "fixture-session", "update": {"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": value}}}})

for line in sys.stdin:
    message = json.loads(line)
    method = message.get("method")
    identifier = message.get("id")
    if method == "initialize":
        reply(identifier, {"protocolVersion": 1, "agentCapabilities": {}, "agentInfo": {"name": "acp-fixture", "title": "ACP test agent"}, "authMethods": []})
    elif method == "session/new":
        assert message["params"]["mcpServers"][0]["name"] == "schema-atlas"
        reply(identifier, {"sessionId": "fixture-session"})
    elif method == "session/prompt":
        prompt = message["params"]["prompt"][0]["text"].split("\n\n", 1)[-1].lower()
        if "permission" in prompt:
            permission_prompt = identifier
            send({"id": "permission-1", "method": "session/request_permission", "params": {"sessionId": "fixture-session", "toolCall": {"toolCallId": "test-call", "title": "Test permission request", "kind": "read", "status": "pending"}, "options": [{"optionId": "allow", "name": "Allow once", "kind": "allow_once"}, {"optionId": "reject", "name": "Reject", "kind": "reject_once"}]}})
        elif "activity" in prompt:
            pending = identifier
            def update(value):
                send({"method": "session/update", "params": {"sessionId": "fixture-session", "update": value}})
            update({"sessionUpdate": "agent_thought_chunk", "content": {"type": "text", "text": "Private fixture reasoning"}})
            update({"sessionUpdate": "plan", "entries": []})
            update({"sessionUpdate": "tool_call", "toolCallId": "activity-tool", "title": "Terminal", "status": "pending"})
            update({"sessionUpdate": "tool_call_update", "toolCallId": "activity-tool", "title": "Preparing layout"})
        elif "wait" in prompt:
            pending = identifier
            text("Waiting for cancellation…")
        else:
            text("ACP streaming ")
            text("works. This is a deterministic test agent, not an AI model.")
            reply(identifier, {"stopReason": "end_turn"})
    elif method == "session/cancel":
        if pending is not None:
            reply(pending, {"stopReason": "cancelled"})
            pending = None
    elif identifier == "permission-1":
        text("Permission outcome: " + json.dumps(message.get("result")))
        reply(permission_prompt, {"stopReason": "end_turn"})
        permission_prompt = None
    elif identifier is not None:
        send({"id": identifier, "error": {"code": -32601, "message": "Unsupported fixture method"}})
