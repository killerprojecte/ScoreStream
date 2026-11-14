#!/usr/bin/env python3
"""Real HTTP+SSE acceptance; requires permission to bind a loopback TCP socket."""
import http.client
import json
import os
from pathlib import Path
import re
import secrets
import signal
import socket
import subprocess
import sys
import time

root = Path(__file__).resolve().parents[1]
binary = root / 'target/debug/scorestream-mcp'
if sys.platform == 'win32':
    binary = binary.with_suffix('.exe')
token = secrets.token_urlsafe(32)
env = dict(os.environ, SCORESTREAM_TEST_TOKEN=token)
proc = subprocess.Popen([str(binary), '--transport', 'sse', '--port', '0', '--token-env',
                         'SCORESTREAM_TEST_TOKEN'], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                        text=True, env=env, cwd=root)
streams = []

def event(response):
    kind, data = None, []
    while True:
        line = response.readline().decode().rstrip('\r\n')
        if not line:
            if kind is not None:
                return kind, '\n'.join(data)
            raise AssertionError('SSE ended before an event')
        if line.startswith('event:'):
            kind = line[6:].strip()
        if line.startswith('data:'):
            data.append(line[5:].lstrip())

try:
    line = proc.stderr.readline()
    assert token not in line
    match = re.search(r'http://127\.0\.0\.1:(\d+)/sse', line)
    assert match, f'Server did not start: {line.strip()} {proc.stderr.read() if proc.poll() is not None else ""}'
    port = int(match[1])
    def request(method, path, body=None, auth=True, origin=None):
        connection = http.client.HTTPConnection('127.0.0.1', port, timeout=5)
        headers = {'Accept': 'text/event-stream', 'Content-Type': 'application/json'}
        if auth:
            headers['Authorization'] = f'Bearer {token if auth is True else auth}'
        if origin is not None:
            headers['Origin'] = origin
        connection.request(method, path, body, headers)
        response = connection.getresponse()
        if method == 'GET' and response.status == 200:
            streams.append((connection, response))
            return response
        status, data = response.status, response.read()
        connection.close()
        return status, data

    occupied = subprocess.run([str(binary), '--transport', 'sse', '--port', str(port)],
                              capture_output=True, text=True, timeout=5)
    assert occupied.returncode != 0 and occupied.stdout == ''
    invalid = subprocess.run([str(binary), '--transport', 'sse', '--port', '0', '--token-env',
                              'SCORESTREAM_TEST_TOKEN'], capture_output=True, text=True, timeout=5,
                             env=dict(env, SCORESTREAM_TEST_TOKEN='bad token'))
    assert invalid.returncode != 0 and 'bad token' not in invalid.stderr
    assert request('GET', '/sse', auth=False)[0] == 401
    assert request('GET', '/sse', auth='wrong-token')[0] == 401
    assert request('GET', '/sse', origin='https://untrusted.example')[0] == 403
    a = request('GET', '/sse')
    b = request('GET', '/sse', origin=f'http://localhost:{port}')
    assert a.headers['Content-Type'].startswith('text/event-stream')
    kind_a, endpoint_a = event(a)
    kind_b, endpoint_b = event(b)
    assert kind_a == kind_b == 'endpoint' and endpoint_a != endpoint_b
    assert token not in endpoint_a + endpoint_b
    assert request('POST', endpoint_a, '{}', auth=False)[0] == 401
    assert request('POST', endpoint_a, '{}', origin='null')[0] == 403
    assert request('POST', endpoint_a, '{')[0] == 400
    requests = [
        {'jsonrpc': '2.0', 'id': 1, 'method': 'initialize', 'params': {'protocolVersion': '2024-11-05', 'capabilities': {}, 'clientInfo': {'name': 'http-acceptance', 'version': '1'}}},
        {'jsonrpc': '2.0', 'method': 'notifications/initialized'},
        {'jsonrpc': '2.0', 'id': 2, 'method': 'tools/list'},
        {'jsonrpc': '2.0', 'id': 3, 'method': 'tools/call', 'params': {'name': 'score_get_format', 'arguments': {}}},
        {'jsonrpc': '2.0', 'id': 4, 'method': 'resources/read', 'params': {'uri': 'scorestream://spec/1.1/grammar'}},
        {'jsonrpc': '2.0', 'id': 5, 'method': 'resources/read', 'params': {'uri': 'scorestream://workspace/current'}},
    ]
    format_variants = [
        {'detail': 'quick'}, {'detail': 'full'}, {'topics': []},
        {'topics': ['time', 'relations']}, {'topics': ['editing']},
        {'topics': ['unknown']},
    ]
    requests.extend({'jsonrpc': '2.0', 'id': 10 + index, 'method': 'tools/call',
                     'params': {'name': 'score_get_format', 'arguments': args}}
                    for index, args in enumerate(format_variants))
    baseline = subprocess.run([str(binary)], input='\n'.join(map(json.dumps, requests))+'\n', capture_output=True, text=True, check=True)
    expected = [json.loads(v) for v in baseline.stdout.splitlines()]
    for stream, endpoint in [(a, endpoint_a), (b, endpoint_b)]:
        answers = []
        for message in requests:
            assert request('POST', endpoint, json.dumps(message)) == (202, b'')
            if 'id' in message:
                kind, data = event(stream)
                assert kind == 'message'
                answers.append(json.loads(data))
        assert answers == expected
        assert [v['name'] for v in answers[1]['result']['tools']] == ['score_get_format']
    # Closing one stream invalidates its session without affecting the other.
    streams[0][1].close()
    streams[0][0].close()
    ping = json.dumps({'jsonrpc': '2.0', 'id': 99, 'method': 'ping'})
    for _ in range(30):
        status, _ = request('POST', endpoint_a, ping)
        if status in (404, 410):
            break
        time.sleep(.05)
    assert status in (404, 410), status
    assert request('POST', endpoint_b, ping)[0] == 202
    assert json.loads(event(b)[1])['id'] == 99
    # SIGINT stops with an active SSE client, without requiring a client close.
    proc.send_signal(signal.SIGINT)
    stdout, stderr = proc.communicate(timeout=5)
    assert proc.returncode == 0, stderr
    assert stdout == '' and token not in stderr
    # Restarting the service on the same port does not resurrect old sessions.
    proc = subprocess.Popen([str(binary), '--transport', 'sse', '--port', str(port), '--token-env',
                             'SCORESTREAM_TEST_TOKEN'], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            text=True, env=env, cwd=root)
    line = proc.stderr.readline()
    assert f'http://127.0.0.1:{port}/sse' in line and token not in line
    assert request('POST', endpoint_b, ping)[0] == 404
    proc.send_signal(signal.SIGINT)
    stdout, stderr = proc.communicate(timeout=5)
    assert proc.returncode == 0 and stdout == '' and token not in stderr
    with socket.socket() as check:
        check.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        check.bind(('127.0.0.1', port))
    print(json.dumps({'status': 'PASS', 'transport': 'legacy HTTP+SSE', 'sessions': 2,
                      'stdio_parity': True, 'initialize_tools_resources': True,
                      'additional_format_variants': len(format_variants),
                      'auth_and_origin': True, 'disconnect_cleanup': True,
                      'shutdown_with_active_stream': True, 'port_released': True,
                      'occupied_port_rejected': True, 'invalid_token_rejected': True,
                      'stale_session_after_restart_rejected': True}, indent=2))
finally:
    for connection, response in streams:
        response.close()
        connection.close()
    if proc.poll() is None:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait()
