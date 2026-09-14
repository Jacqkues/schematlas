#!/usr/bin/env python3
"""Exercise the built app's --mcp mode against an isolated loopback bridge."""
import http.server
import json
import os
import secrets
import subprocess
import sys
import threading

token = secrets.token_hex(32)
requests = []

class Bridge(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        assert self.path == '/tool'
        assert self.headers['Authorization'] == 'Bearer ' + token
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        requests.append(body)
        name = body.get('name')
        result = {'result': 'db  Disposable source — sqlite, 1 entities, 0 relationships; schemas: main'} if name == 'list_sources' else {'result': {'echo': name}}
        self.send_response(200)
        self.send_header('Content-Type','application/json')
        self.end_headers()
        self.wfile.write(json.dumps(result).encode())
    def log_message(self, *_):
        pass

server = http.server.ThreadingHTTPServer(('127.0.0.1',0), Bridge)
threading.Thread(target=server.serve_forever,daemon=True).start()
process = subprocess.Popen([sys.argv[1],'--mcp'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env={**os.environ,'SCHEMA_ATLAS_ENDPOINT':f'http://127.0.0.1:{server.server_port}/tool','SCHEMA_ATLAS_TOKEN':token})
def send(message):
    process.stdin.write(json.dumps({'jsonrpc':'2.0',**message})+'\n')
    process.stdin.flush()
def rpc(identifier,method,params):
    send({'id':identifier,'method':method,'params':params})
    while True:
        line=process.stdout.readline()
        assert line, 'MCP process exited unexpectedly'
        result=json.loads(line)
        if result.get('id')==identifier:
            assert 'error' not in result, result
            return result['result']
try:
    result=rpc(1,'initialize',{'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'schema-atlas-test','version':'1'}})
    assert 'tools' in result['capabilities']
    result_instructions = result.get('instructions','')
    send({'method':'notifications/initialized'})
    tools=rpc(2,'tools/list',{})['tools']
    assert {t['name'] for t in tools} == {'list_sources','search_schema','describe_table','get_schema','find_join_path','table_stats','sample_rows','explain_sql','query_sql','request_http','get_canvas','move_nodes','create_group','remove_group'}
    for tool in tools:
        assert tool['inputSchema']['type']=='object'
    assert 'search_schema' in result_instructions, 'usage guidance must ship as MCP instructions'
    result=rpc(3,'tools/call',{'name':'list_sources','arguments':{}})
    assert not result.get('isError',False)
    # Text results reach the model verbatim: no JSON quoting, no escaped newlines.
    assert result['content'][0]['text'].startswith('db  Disposable source'), result
    result=rpc(4,'tools/call',{'name':'search_schema','arguments':{'query':'order'}})
    assert json.loads(result['content'][0]['text'])=={'echo':'search_schema'}
    assert requests == [{'name':'list_sources','arguments':{}},{'name':'search_schema','arguments':{'query':'order'}}]
    print('PASS: MCP initialization, 14 tool schemas, server instructions, authenticated proxy, verbatim text results, stdio-only mode')
finally:
    process.stdin.close()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
    server.shutdown()
