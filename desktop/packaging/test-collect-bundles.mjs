import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { bundleFormats, collectBundles } from './collect-bundles.mjs';

const commit = 'a'.repeat(40);
function fixture(t, target) {
  const root = mkdtempSync(join(tmpdir(), 'scorestream-bundles-test-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const put = (path, content = 'fixture bytes') => {
    const file = join(root, path);
    mkdirSync(dirname(file), { recursive: true });
    writeFileSync(file, content);
    return file;
  };
  for (const path of ['Cargo.lock', 'crates/scorestream-desktop/Cargo.lock', 'desktop/package-lock.json',
    'desktop/THIRD_PARTY_NOTICES.md', 'desktop/licenses/test.txt']) put(path);
  const bundleRoot = `crates/scorestream-desktop/target/${target}/release/bundle`;
  const files = (bundleFormats[target] ?? []).map(([folder, suffix]) => put(`${bundleRoot}/${folder}/ScoreStream${suffix}`));
  if (target.endsWith('apple-darwin')) put(`${bundleRoot}/macos/ScoreStream.app/Contents/MacOS/scorestream-desktop`);
  return { root, target, output: join(root, 'out'), commit, files, put, bundleRoot };
}

for (const target of Object.keys(bundleFormats)) {
  test(`collects all expected ${target} installers and verifies checksums`, (t) => {
    const input = fixture(t, target);
    const manifest = collectBundles(input);
    assert.equal(manifest.target, target);
    assert.equal(manifest.commit, commit);
    assert.equal(manifest.installers.length, bundleFormats[target].length);
    for (const file of manifest.installers) {
      const bytes = readFileSync(join(input.output, file.name));
      assert.equal(file.sha256, createHash('sha256').update(bytes).digest('hex'));
      assert.equal(file.bytes, bytes.length);
      assert.match(readFileSync(join(input.output, 'SHA256SUMS'), 'utf8'), new RegExp(file.sha256));
    }
    assert.ok(existsSync(join(input.output, 'licenses/test.txt')));
    assert.deepEqual(JSON.parse(readFileSync(join(input.output, 'build-info.json'), 'utf8')), manifest);
  });
}

for (const target of ['x86_64-unknown-linux-gnu', 'aarch64-unknown-linux-gnu']) {
  for (const [index, [, suffix]] of bundleFormats[target].entries()) {
    test(`fails when ${target} is missing ${suffix}, even when the other format exists`, (t) => {
      const input = fixture(t, target);
      rmSync(input.files[index]);
      assert.throws(() => collectBundles(input), /Expected exactly one/);
      assert.ok(!existsSync(input.output));
    });
  }
}

test('does not treat a raw Windows executable as an installer', (t) => {
  const input = fixture(t, 'x86_64-pc-windows-msvc');
  rmSync(input.files[0]);
  input.put(`${input.bundleRoot}/nsis/scorestream-desktop.exe`);
  assert.throws(() => collectBundles(input), /Expected exactly one -setup.exe/);
});

test('fails on an empty installer', (t) => {
  const input = fixture(t, 'aarch64-pc-windows-msvc');
  writeFileSync(input.files[0], '');
  assert.throws(() => collectBundles(input), /not a nonempty file/);
});

test('fails on a directory masquerading as an installer', (t) => {
  const input = fixture(t, 'aarch64-pc-windows-msvc');
  rmSync(input.files[0]);
  mkdirSync(input.files[0]);
  assert.throws(() => collectBundles(input), /not a nonempty file/);
});

test('fails on ambiguous stale installers', (t) => {
  const input = fixture(t, 'x86_64-unknown-linux-gnu');
  input.put(`${input.bundleRoot}/deb/older.deb`);
  assert.throws(() => collectBundles(input), /found 2/);
});

test('requires a native macOS app alongside the DMG', (t) => {
  const input = fixture(t, 'aarch64-apple-darwin');
  rmSync(join(input.root, input.bundleRoot, 'macos'), { recursive: true });
  assert.throws(() => collectBundles(input), /Missing native macOS application binary/);
});

test('rejects unknown targets and incomplete provenance', (t) => {
  const input = fixture(t, 'x86_64-unknown-linux-gnu');
  assert.throws(() => collectBundles({ ...input, target: 'universal-apple-darwin' }), /Unsupported/);
  assert.throws(() => collectBundles({ ...input, commit: '1234567' }), /full source commit/);
});

test('refuses to mix outputs with a previous collection', (t) => {
  const input = fixture(t, 'x86_64-unknown-linux-gnu');
  collectBundles(input);
  assert.throws(() => collectBundles(input), /Output already exists/);
});
