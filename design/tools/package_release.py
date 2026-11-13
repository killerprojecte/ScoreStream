#!/usr/bin/env python3
"""Rebuild and check this design deliverable. Not ScoreStream product code."""
from pathlib import Path
from datetime import datetime, timezone
import hashlib
import json
import re
import shutil
import subprocess
import sys
import zipfile

from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT.parent / 'deliverables'


def emit_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


def package_files():
    return sorted(p for p in ROOT.rglob('*') if p.is_file()
                  and '__pycache__' not in p.parts
                  and not any(x.startswith('.') for x in p.relative_to(ROOT).parts)
                  and p.suffix != '.pyc')


def relative_links(text, source):
    """Rebase ordinary inline Markdown links for the combined root document."""
    def replace(match):
        target = match.group(2)
        if re.match(r'^[a-zA-Z][a-zA-Z0-9+.-]*:', target) or target.startswith('#'):
            return match.group(0)
        file, marker, anchor = target.partition('#')
        resolved = (source.parent / file).resolve()
        if resolved.is_relative_to(ROOT):
            return '[' + match.group(1) + '](' + str(resolved.relative_to(ROOT)) + (marker + anchor if marker else '') + ')'
        return match.group(0)
    return re.sub(r'\[([^\]]+)\]\(([^\s)]+)\)', replace, text)


def combine():
    chapters = sorted((ROOT / 'docs').glob('*.md')) + [ROOT / 'spec/agent-reference.md']
    toc = []
    bodies = []
    for path in chapters:
        body = path.read_text(encoding='utf-8')
        title = body.splitlines()[0].removeprefix('# ')
        anchor = re.sub(r'[^\w\s-]', '', title.lower())
        anchor = re.sub(r'\s+', '-', anchor)
        toc.append(f'- [{title}](#{anchor})')
        fenced = False
        lines = []
        for line in relative_links(body, path).splitlines():
            if line.startswith('```'):
                fenced = not fenced
            if not fenced and line.startswith('#'):
                line = '#' + line
            lines.append(line)
        bodies.append('\n'.join(lines))
    header = '''# ScoreStream 1.1 — English Design Document

**Proposed design baseline:** 2026-11-13. **Product:** Rust desktop score editor with live preview and a shared MCP service for agents.

This document combines the architecture, actual ScoreStream language standard, MCP contract, desktop editor/preview plan, future independent annotation model, Rust implementation milestones, and agent quick reference. The accompanying ZIP contains the EBNF, JSON format descriptor, AST and tool schemas, complete fixtures, and validation utilities. No PDF is provided.

**Delivery status:** design and contracts with finite package QA. No compiled Rust application, running desktop editor, converter, renderer integration, MCP server, annotation database, or playback engine is claimed. Annotation storage and playback are explicitly deferred. Relative file references refer to files inside the accompanying design package.

## Contents

'''
    (ROOT / 'ScoreStream_Design.md').write_text(header + '\n'.join(toc) + '\n\n---\n\n' + '\n\n---\n\n'.join(bodies) + '\n', encoding='utf-8')


def main():
    commands = []
    for script in ['build_format_reference.py', 'build_mcp_catalog.py', 'validate_package.py']:
        result = subprocess.run([sys.executable, str(ROOT / 'tools' / script)], cwd=ROOT,
                                capture_output=True, text=True, check=False)
        commands.append({'command': f'python3 tools/{script}', 'exit_code': result.returncode,
                         'stdout': result.stdout.strip(), 'stderr': result.stderr.strip()})
        if result.returncode:
            raise RuntimeError(f'{script}: {result.stderr}\n{result.stdout}')
    combine()
    schema_count = 0
    json_count = 0
    link_count = 0
    for path in package_files():
        if path.suffix == '.pdf':
            raise RuntimeError('The user requested no PDF.')
        if path.suffix == '.json':
            data = json.loads(path.read_text(encoding='utf-8'))
            json_count += 1
            if path.name.endswith('.schema.json'):
                Draft202012Validator.check_schema(data)
                schema_count += 1
        if path.suffix == '.md':
            for target in re.findall(r'\[[^\]]+\]\(([^\s)]+)\)', path.read_text(encoding='utf-8')):
                if re.match(r'^[a-zA-Z][a-zA-Z0-9+.-]*:', target) or target.startswith('#'):
                    continue
                filename = target.split('#', 1)[0]
                if not (path.parent / filename).exists():
                    raise RuntimeError(f'Broken local link in {path.relative_to(ROOT)}: {target}')
                link_count += 1
    timestamp = datetime.now(timezone.utc).isoformat()
    report = {
        'package': 'ScoreStream 1.1 design', 'checked_at_utc': timestamp, 'status': 'passed',
        'executed_commands': commands, 'json_files_parsed': json_count,
        'json_schemas_checked': schema_count, 'local_markdown_links_checked': link_count,
        'pdf_files': 0,
        'limits': ['Finite design-package checks only; see validation/README.md.',
                   'No Rust compilation or product runtime executed.',
                   'No MusicXML conversion, GUI, renderer, actual MCP server, annotations, or playback tested.']
    }
    emit_json(ROOT / 'validation/release-checks.json', report)
    entries = []
    for path in package_files():
        if path.name == 'MANIFEST.json':
            continue
        content = path.read_bytes()
        entries.append({'path': path.relative_to(ROOT).as_posix(), 'size_bytes': len(content),
                        'sha256': hashlib.sha256(content).hexdigest()})
    emit_json(ROOT / 'MANIFEST.json', {
        'name': 'ScoreStream Rust Desktop and Agent Design Package', 'version': '1.1',
        'status': 'proposed design; no product implementation', 'language': 'English',
        'built_at_utc': timestamp, 'hash_algorithm': 'SHA-256',
        'manifest_self_hash_omitted': True, 'files': entries
    })
    OUT.mkdir(exist_ok=True)
    archive = OUT / 'ScoreStream_1.1_Design_Package.zip'
    with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for path in package_files():
            z.write(path, str(Path(ROOT.name) / path.relative_to(ROOT)))
    with zipfile.ZipFile(archive) as z:
        bad = z.testzip()
        if bad:
            raise RuntimeError('ZIP CRC failure: ' + bad)
        archive_files = len(z.namelist())
    shutil.copyfile(ROOT / 'ScoreStream_Design.md', OUT / 'ScoreStream_Design.md')
    shutil.copyfile(ROOT / 'spec/scorestream-language.json', OUT / 'ScoreStream_Format_1.1.json')
    shutil.copyfile(ROOT / 'spec/agent-reference.md', OUT / 'ScoreStream_Agent_Reference.md')
    print(json.dumps({'status': 'passed', 'archive_files': archive_files,
                      'schemas_checked': schema_count, 'local_links_checked': link_count,
                      'outputs': [{'file': str(p), 'bytes': p.stat().st_size} for p in sorted(OUT.iterdir()) if p.is_file()]}, indent=2))


if __name__ == '__main__':
    main()
