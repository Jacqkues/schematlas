#!/usr/bin/env python3
"""Deterministic ACP fixture, not an AI model. Use Python as the executable.
Arguments: ["/absolute/path/to/mock-acp-agent.py"]
Prompts containing 'permission' exercise permission cards; 'wait' exercises cancel;
'blocks' exercises non-text content; 'refuse' and 'truncate' exercise stop reasons.
Only 'fixture-session' can be resumed; session/load of any other id is refused,
which exercises the client's fallback to a new session.
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
        reply(identifier, {"protocolVersion": 1, "agentCapabilities": {"loadSession": True}, "agentInfo": {"name": "acp-fixture", "title": "ACP test agent"}, "authMethods": []})
    elif method == "session/new":
        assert message["params"]["mcpServers"][0]["name"] == "schema-atlas"
        reply(identifier, {"sessionId": "fixture-session"})
    elif method == "session/load":
        assert message["params"]["mcpServers"][0]["name"] == "schema-atlas"
        if message["params"].get("sessionId") != "fixture-session":
            send({"id": identifier, "error": {"code": -32602, "message": "Unknown session"}})
            continue
        # Replay the conversation, then answer the request, in that order.
        def replay(value):
            send({"method": "session/update", "params": {"sessionId": "fixture-session", "update": value}})
        replay({"sessionUpdate": "user_message_chunk", "content": {"type": "text", "text": "what tables "}})
        replay({"sessionUpdate": "user_message_chunk", "content": {"type": "text", "text": "are there?"}})
        replay({"sessionUpdate": "tool_call", "toolCallId": "replayed-tool", "title": "get_schema", "status": "completed"})
        replay({"sessionUpdate": "agent_message_chunk", "content": {"type": "text", "text": "Replayed answer."}})
        reply(identifier, {})
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
        elif "blocks" in prompt:
            def update(value):
                send({"method": "session/update", "params": {"sessionId": "fixture-session", "update": value}})
            # Content blocks the panel used to drop on the floor.
            update({"sessionUpdate": "agent_message_chunk", "content": {"type": "image", "mimeType": "image/png", "data": "iVBORw0KGgo="}})
            update({"sessionUpdate": "agent_message_chunk", "content": {"type": "resource_link", "name": "schema.sql", "uri": "file:///tmp/schema.sql"}})
            update({"sessionUpdate": "tool_call", "toolCallId": "blocks-tool", "title": "Read schema", "status": "pending"})
            update({"sessionUpdate": "tool_call_update", "toolCallId": "blocks-tool", "status": "completed", "content": [
                {"type": "content", "content": {"type": "text", "text": "orders: 42 rows"}},
                {"type": "diff", "path": "/tmp/schema.sql", "oldText": "a", "newText": "b"},
            ]})
            reply(identifier, {"stopReason": "end_turn"})
        elif "refuse" in prompt:
            text("I will not.")
            reply(identifier, {"stopReason": "refusal"})
        elif "truncate" in prompt:
            text("Half an ans")
            reply(identifier, {"stopReason": "max_tokens"})
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
