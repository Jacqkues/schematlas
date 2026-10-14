#!/usr/bin/env python3
"""Synthetic native-harness protocol fixture. No AI, credentials, network or tools."""
import json
import sys
import uuid

protocol = sys.argv[sys.argv.index('--native-protocol') + 1]
pending = None
thread = 'codex-fixture-thread'
thread_count = 0
claude_session = next((a.split('=', 1)[1] for a in sys.argv if a.startswith(('--session-id=', '--resume='))), 'claude-fixture-session')

def send(value):
    print(json.dumps(value), flush=True)

def answer(request, result):
    send({'id': request['id'], 'result': result})

def codex_finish(status='completed'):
    turn = {'id': 'turn-fixture', 'status': status}
    if status == 'failed':
        turn['error'] = {'message': 'Synthetic native failure'}
    send({'method': 'turn/completed', 'params': {'threadId': thread, 'turn': turn}})

def codex_text():
    send({'method': 'item/reasoning/textDelta', 'params': {'threadId': thread, 'itemId': 'reason', 'delta': 'PRIVATE THINKING MUST NOT BE RETAINED'}})
    for delta in ['Native ', 'answer.']:
        send({'method': 'item/agentMessage/delta', 'params': {'threadId': thread, 'itemId': 'answer', 'delta': delta}})
    send({'method': 'item/completed', 'params': {'threadId': thread, 'item': {'id': 'answer', 'type': 'agentMessage', 'text': 'Native answer.'}}})
    codex_finish()

def claude_text():
    send({'type': 'stream_event', 'session_id': claude_session, 'event': {'type': 'message_start', 'message': {'id': 'answer'}}})
    for delta in [{'type': 'thinking_delta', 'thinking': 'PRIVATE THINKING MUST NOT BE RETAINED'}, {'type': 'text_delta', 'text': 'Native '}, {'type': 'text_delta', 'text': 'answer.'}]:
        send({'type': 'stream_event', 'session_id': claude_session, 'event': {'type': 'content_block_delta', 'delta': delta}})
    send({'type': 'assistant', 'session_id': claude_session, 'message': {'id': 'answer', 'content': [{'type': 'text', 'text': 'Native answer.'}]}})
    send({'type': 'result', 'subtype': 'success', 'is_error': False, 'session_id': claude_session})

for line in sys.stdin:
    request = json.loads(line)
    if protocol == 'codex':
        method = request.get('method')
        if method == 'initialize':
            answer(request, {'userAgent': 'Synthetic Codex native fixture'})
        elif method == 'model/list':
            answer(request, {'data': [{'model': 'fixture-fast', 'displayName': 'Fixture Fast'}, {'model': 'fixture-deep', 'displayName': 'Fixture Deep'}], 'nextCursor': None})
        elif method == 'account/read':
            answer(request, {'requiresOpenaiAuth': True, 'account': None if '--needs-login' in sys.argv else {'type': 'chatgpt'}})
        elif method in ('thread/start', 'thread/resume'):
            if '--fail-thread' in sys.argv:
                send({'id': request['id'], 'error': {'message': 'Synthetic thread startup failure'}})
                continue
            params = request['params']
            assert params['config']['mcp_servers']['schema-atlas']['args'] == ['--mcp']
            assert params['sandbox'] == 'read-only'
            assert params['approvalPolicy'] == 'on-request'
            if method == 'thread/start':
                thread_count += 1
                thread = 'codex-fixture-thread' if thread_count == 1 else 'codex-fixture-thread-' + str(uuid.uuid4())
            elif params['threadId'].startswith('codex-fixture-thread'):
                thread = params['threadId']
            if method == 'thread/resume' and params['threadId'] != thread:
                send({'id': request['id'], 'error': {'message': 'Unknown thread'}})
            else:
                turns = [] if method == 'thread/start' else [{'items': [{'type': 'userMessage', 'content': [{'type': 'text', 'text': 'Previous question'}]}, {'type': 'agentMessage', 'text': 'Replayed native answer.'}]}]
                answer(request, {'model': 'fixture-fast', 'thread': {'id': thread, 'turns': turns}})
        elif method == 'turn/start':
            text = request['params']['input'][0]['text']
            if text == 'verify model':
                assert request['params']['model'] == 'fixture-deep'
            answer(request, {'turn': {'id': 'turn-fixture', 'status': 'inProgress'}})
            send({'method': 'turn/started', 'params': {'threadId': thread, 'turn': {'id': 'turn-fixture'}}})
            if text == 'request approval':
                pending = True
                send({'method': 'item/commandExecution/requestApproval', 'id': 700, 'params': {'threadId': thread, 'turnId': 'turn-fixture', 'itemId': 'command', 'command': 'synthetic-command'}})
            elif text == 'cancel me':
                pending = True
            elif text == 'fail':
                codex_finish('failed')
            else:
                codex_text()
        elif method == 'turn/interrupt':
            answer(request, {})
            codex_finish('interrupted')
        elif request.get('id') == 700:
            assert request['result']['decision'] in ('accept', 'decline')
            codex_text()
    else:
        kind = request.get('type')
        if kind == 'control_request':
            subtype = request['request']['subtype']
            if subtype == 'initialize' and '--resume=forgotten' in sys.argv:
                send({'type': 'control_response', 'response': {'subtype': 'error', 'request_id': request['request_id'], 'error': 'Unknown native session'}})
                continue
            data = {'models': [{'value': 'fixture-fast', 'displayName': 'Fixture Fast'}, {'value': 'fixture-deep', 'displayName': 'Fixture Deep'}], 'model': 'fixture-fast'} if subtype == 'initialize' else {}
            if subtype == 'set_model':
                assert request['request']['model'] in ('fixture-fast', 'fixture-deep')
            send({'type': 'control_response', 'response': {'subtype': 'success', 'request_id': request['request_id'], 'response': data}})
            if subtype == 'interrupt':
                send({'type': 'result', 'subtype': 'success', 'is_error': False, 'session_id': claude_session})
        elif kind == 'user':
            text = request['message']['content'][0]['text']
            if text == 'request approval':
                send({'type': 'control_request', 'request_id': 'permission', 'request': {'subtype': 'can_use_tool', 'tool_name': 'Bash', 'input': {'command': 'synthetic-command'}}})
            elif text == 'cancel me':
                pending = True
            elif text == 'fail':
                send({'type': 'result', 'subtype': 'error_during_execution', 'is_error': True, 'errors': ['Synthetic native failure']})
            else:
                claude_text()
        elif kind == 'control_response':
            assert request['response']['response']['behavior'] in ('allow', 'deny')
            claude_text()
