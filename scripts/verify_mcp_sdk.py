#!/usr/bin/env python3
"""Optional independent interoperability test. Requires the official `mcp` Python SDK."""
import asyncio
import json
import os
from pathlib import Path
import re
import secrets
import signal
import subprocess
from importlib.metadata import version
import httpx
from mcp import ClientSession
from mcp.client.sse import sse_client

root = Path(__file__).resolve().parents[1]
binary = root / 'target/debug/scorestream-mcp'
if os.name == 'nt':
    binary = binary.with_suffix('.exe')
token = secrets.token_urlsafe(32)
proc = subprocess.Popen([str(binary), '--transport', 'sse', '--port', '0', '--token-env', 'MCP_TEST_KEY'], env=dict(os.environ, MCP_TEST_KEY=token), stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
try:
    line = proc.stderr.readline()
    url = re.search(r'http://127\.0\.0\.1:\d+/sse', line)[0]
    # This test only connects to its own loopback listener, never an external proxy.
    def local_client(headers=None, timeout=None, auth=None):
        return httpx.AsyncClient(headers=headers, timeout=timeout, auth=auth, trust_env=False)
    async def client():
        async with sse_client(url, headers={'Authorization': f'Bearer {token}'}, httpx_client_factory=local_client) as streams:
            async with ClientSession(*streams) as session:
                info = await session.initialize()
                tools = await session.list_tools()
                assert [tool.name for tool in tools.tools] == ['score_get_format']
                result = await session.call_tool('score_get_format', {})
                assert not result.isError and result.content
                resources = await session.list_resources()
                assert len(resources.resources) == 5
                grammar = await session.read_resource('scorestream://spec/1.1/grammar')
                assert 'document =' in grammar.contents[0].text
                await session.send_ping()
                return info.protocolVersion
    async def main():
        return await asyncio.gather(client(), client())
    negotiated = asyncio.run(main())
    print(json.dumps({'status':'PASS','client':'official Python MCP SDK','sdk_version':version('mcp'),'simultaneous_clients':2,'negotiated_versions':negotiated,'checks':['initialize','tools/list','tools/call','resources/list','resources/read','ping','Bearer GET and POST']},indent=2))
finally:
    proc.send_signal(signal.SIGINT)
    out, err = proc.communicate(timeout=5)
    assert proc.returncode == 0 and out == '' and token not in err
