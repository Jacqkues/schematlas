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
        result = {'result': {'sources': [{'id':'fixture','name':'Disposable source'}]}}
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
    send({'method':'notifications/initialized'})
    tools=rpc(2,'tools/list',{})['tools']
    assert {t['name'] for t in tools} == {'list_sources','get_schema','query_sql','request_http','get_canvas','move_nodes','create_group','remove_group'}
    for tool in tools:
        assert tool['inputSchema']['type']=='object'
    result=rpc(3,'tools/call',{'name':'list_sources','arguments':{}})
    assert not result.get('isError',False)
    assert json.loads(result['content'][0]['text'])['sources'][0]['id']=='fixture'
    assert requests == [{'name':'list_sources','arguments':{}}]
    print('PASS: MCP initialization, tool schemas, authenticated proxy, JSON result, stdio-only mode')
finally:
    process.stdin.close()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()
    server.shutdown()
