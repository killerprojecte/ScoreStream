// Collect installers only: never substitute a raw executable or frontend dist.
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const bundleFormats = Object.freeze({
  'x86_64-unknown-linux-gnu': [['deb', '.deb'], ['appimage', '.AppImage']],
  'aarch64-unknown-linux-gnu': [['deb', '.deb'], ['appimage', '.AppImage']],
  'x86_64-apple-darwin': [['dmg', '.dmg']],
  'aarch64-apple-darwin': [['dmg', '.dmg']],
  'x86_64-pc-windows-msvc': [['nsis', '-setup.exe']],
  'aarch64-pc-windows-msvc': [['nsis', '-setup.exe']],
});

function sha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

export function collectBundles({ root, target, output, commit, toolchain = {} }) {
  const formats = bundleFormats[target];
  if (!formats) throw new Error(`Unsupported desktop target: ${target}`);
  if (!/^[a-f0-9]{40}$/.test(commit)) throw new Error('A full source commit SHA is required');
  if (existsSync(output)) throw new Error(`Output already exists; use a fresh directory: ${output}`);

  const bundleRoot = join(root, 'crates/scorestream-desktop/target', target, 'release/bundle');
  const files = formats.map(([folder, suffix]) => {
    const directory = join(bundleRoot, folder);
    const matches = existsSync(directory)
      ? readdirSync(directory).filter((name) => name.endsWith(suffix))
      : [];
    if (matches.length !== 1) {
      throw new Error(`Expected exactly one ${suffix} installer in ${directory}; found ${matches.length}`);
    }
    const source = join(directory, matches[0]);
    if (!statSync(source).isFile() || statSync(source).size === 0) {
      throw new Error(`Installer is not a nonempty file: ${source}`);
    }
    return { name: matches[0], source };
  });

  // A DMG must come from a real Tauri .app, not a renamed executable.
  if (target.endsWith('apple-darwin')) {
    const binary = join(bundleRoot, 'macos/ScoreStream.app/Contents/MacOS/scorestream-desktop');
    if (!existsSync(binary) || !statSync(binary).isFile() || statSync(binary).size === 0) {
      throw new Error(`Missing native macOS application binary: ${binary}`);
    }
  }

  const lockfiles = ['Cargo.lock', 'crates/scorestream-desktop/Cargo.lock', 'desktop/package-lock.json'];
  const lockHashes = Object.fromEntries(lockfiles.map((path) => [path, sha256(join(root, path))]));
  mkdirSync(output, { recursive: true });
  for (const { name, source } of files) copyFileSync(source, join(output, name));
  copyFileSync(join(root, 'desktop/THIRD_PARTY_NOTICES.md'), join(output, 'THIRD_PARTY_NOTICES.md'));
  cpSync(join(root, 'desktop/licenses'), join(output, 'licenses'), { recursive: true });
  const manifest = {
    schemaVersion: 1,
    commit,
    target,
    purpose: 'Unsigned CI build for testing; not a signed, notarized or release-qualified distribution.',
    toolchain,
    lockfiles: lockHashes,
    installers: files.map(({ name, source }) => ({
      name, source: relative(root, source).split('\\').join('/'),
      bytes: statSync(source).size, sha256: sha256(source),
    })),
  };
  writeFileSync(join(output, 'build-info.json'), `${JSON.stringify(manifest, null, 2)}\n`);
  writeFileSync(join(output, 'SHA256SUMS'), manifest.installers.map(({ sha256, name }) => `${sha256}  ${name}\n`).join(''));
  writeFileSync(join(output, 'README.txt'), [
    `ScoreStream CI installers for ${target}`,
    `Source commit: ${commit}`,
    'These installers have not been signed, notarized, or qualified by clean-host runtime tests.',
    'Do not disable OS security to bypass an installation warning.',
    'Linux: GitHub artifact ZIPs do not preserve executable mode. Run chmod +x on the AppImage after extracting.',
    'Linux baseline: Ubuntu 22.04. Compatibility with other distributions must be tested.',
    'Windows: NSIS may download WebView2 at installation if it is not already installed.',
    'Windows ARM64: the app is native ARM64; the NSIS installer runs as x86 under Windows emulation.',
    'macOS: each DMG contains a single-architecture .app; these are not universal binaries.',
    'Frontend third-party notices are included. Full bundled dependency/source compliance remains release work.',
    '',
  ].join('\n'));
  return manifest;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [target, output] = process.argv.slice(2);
    if (!target || !output) throw new Error('Usage: node collect-bundles.mjs <rust-target> <fresh-output-directory>');
    const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
    const command = (name, args) => execFileSync(name, args, { cwd: root, encoding: 'utf8' }).trim();
    const manifest = collectBundles({
      root, target, output: resolve(output),
      commit: command('git', ['rev-parse', 'HEAD']),
      toolchain: {
        node: process.version,
        rustc: command('rustc', ['--version']),
        cargo: command('cargo', ['--version']),
        runner: process.env.RUNNER_OS ?? process.platform,
        runnerArch: process.env.RUNNER_ARCH ?? process.arch,
        image: process.env.ImageOS ?? null,
        imageVersion: process.env.ImageVersion ?? null,
      },
    });
    console.log(JSON.stringify(manifest, null, 2));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
