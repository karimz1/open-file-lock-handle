import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

const script = fileURLToPath(new URL('./render-release-notes.mjs', import.meta.url));
const expectedFiles = [
  'oflh-desktop.windows.amd64-installer.exe',
  'oflh-desktop.windows.arm64-installer.exe',
  'oflh-desktop.darwin.arm64.dmg',
  'oflh-desktop.darwin.amd64.dmg',
  'oflh-desktop.linux.amd64.deb',
  'oflh-desktop.linux.arm64.deb',
  'oflh-desktop.linux.amd64.rpm',
  'oflh-desktop.linux.arm64.rpm',
  'oflh-desktop.linux.amd64.tar.gz',
  'oflh-desktop.linux.arm64.tar.gz',
  'oflh-cli.windows.amd64.exe',
  'oflh-cli.windows.arm64.exe',
  'oflh-cli.darwin.arm64',
  'oflh-cli.darwin.amd64',
  'oflh-cli.linux.amd64',
  'oflh-cli.linux.arm64',
  'checksums.txt',
];

function render(...args) {
  return spawnSync(process.execPath, [script, ...args], { encoding: 'utf8' });
}

for (const tag of ['v1.2.3', 'v1.2.3-rc.1', 'v1.2.3+build.4']) {
  test(`${tag}: downloads use this release and retain every user-facing filename`, () => {
    const result = render(tag);
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stderr);
    const notes = result.stdout.replaceAll('\r\n', '\n');
    assert.ok(!notes.includes('{{TAG}}'));
    assert.ok(!notes.includes('/latest/download/'));
    const downloads = [...notes.matchAll(/\[([^\]]+)\]\((https:\/\/github\.com\/karimz1\/open-file-lock-handle\/releases\/download\/[^)]+)\)/g)];
    assert.deepEqual([...new Set(downloads.map((match) => match[1]))], expectedFiles);
    assert.equal(downloads.length, expectedFiles.length, 'Download links should not be repeated');
    for (const [, filename, url] of downloads) {
      assert.equal(
        url,
        `https://github.com/karimz1/open-file-lock-handle/releases/download/${encodeURIComponent(tag)}/${filename}`,
      );
    }
    assert.equal(downloads[0][1], 'oflh-desktop.windows.amd64-installer.exe');
    const terminalHeading = notes.indexOf('## Download the terminal version');
    for (const download of downloads) {
      if (download[1].startsWith('oflh-desktop.')) assert.ok(download.index < terminalHeading);
      if (download[1].startsWith('oflh-cli.')) assert.ok(download.index > terminalHeading);
    }
    assert.ok(notes.includes('https://oflh.karimzouine.com#download'));
    assert.ok(notes.includes('[Desktop guide](https://github.com/karimz1/open-file-lock-handle/blob/main/docs/desktop-usage.md)'));
    assert.ok(notes.includes('[Terminal guide](https://github.com/karimz1/open-file-lock-handle/blob/main/docs/terminal-usage.md)'));
    assert.ok(notes.includes('For release candidates, use the links below for this release.'));
    assert.ok(notes.includes('Double-clicking the CLI executable does not open the desktop app.'));
    assert.ok(notes.includes('### macOS installation warning'));
    assert.ok(notes.includes('Because the application is unsigned'));
    assert.ok(notes.includes('```sh\nxattr -cr "/Applications/OFLH Desktop.app"\n```'));
  });
}

test('missing or unsafe tags fail without producing release notes', () => {
  for (const args of [[], ['1.2.3'], ['v1.2.3/other'], ['v1.2.3\nother'], ['v1.2.3', 'extra']]) {
    const result = render(...args);
    assert.ifError(result.error);
    assert.notEqual(result.status, 0);
    assert.equal(result.stdout, '');
    assert.match(result.stderr, /Usage:/);
  }
});
