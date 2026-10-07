const { readFileSync } = require('node:fs');
const { runInNewContext } = require('node:vm');
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { mkdtempSync, writeFileSync, existsSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const { execFileSync } = require('node:child_process');

// Exercise the exact manifest validator used before website commits.
const script = readFileSync(`${__dirname}/sync-updater.sh`, 'utf8');
const validator = script.split("<<'NODE'\n")[1].split('\nNODE')[0];
test('release upload retains the legacy manifest and desktop keeps a fallback', () => {
  const workflow = readFileSync(`${__dirname}/../workflows/release.yml`, 'utf8');
  assert.match(workflow, /cp release-dist\/latest\.json updater-dist\/latest\.json/);
  assert.doesNotMatch(workflow, /mv release-dist\/latest\.json/);
  assert.match(workflow, /gh release upload "\$TAG" release-dist\/\* --clobber/);
  const config = JSON.parse(readFileSync(`${__dirname}/../../crates/oflh-desktop/tauri.conf.json`, 'utf8'));
  assert.deepEqual(config.plugins.updater.endpoints, [
    'https://oflh.karimzouine.com/api/latest.json',
    'https://github.com/karimz1/open-file-lock-handle/releases/latest/download/latest.json',
  ]);
  const cli = readFileSync(`${__dirname}/../../crates/oflh-platform/src/updates.rs`, 'utf8');
  assert.match(cli, /pub const MANIFEST_URL: &str = "https:\/\/oflh\.karimzouine\.com\/api\/latest\.json"/);
  const promotion = readFileSync(`${__dirname}/../workflows/updater-website.yml`, 'utf8');
  assert.match(promotion, /gh release download "\$TAG" --pattern latest\.json/);
  assert.match(promotion, /cmp published-updater\/latest\.json "\$MANIFEST"/);
});
function manifest(tag = 'v1.2.3-rc.1') {
  const platforms = {};
  for (const [os, arch, platformArch] of [
    ['darwin', 'amd64', 'x86_64'],
    ['darwin', 'arm64', 'aarch64'],
    ['windows', 'amd64', 'x86_64'],
    ['windows', 'arm64', 'aarch64'],
  ]) {
    const suffix = os === 'darwin' ? '.app.tar.gz' : '-installer.exe';
    platforms[`${os}-${platformArch}`] = {
      url: `https://github.com/karimz1/open-file-lock-handle/releases/download/${tag}/oflh-desktop.${os}.${arch}${suffix}`,
      signature: 'synthetic-signature',
    };
  }
  return { version: tag.slice(1), platforms };
}
function validate(value, tag = 'v1.2.3-rc.1') {
  runInNewContext(validator, {
    process: { argv: ['node', '-', 'fixture.json', tag] },
    require: () => ({ readFileSync: () => JSON.stringify(value) }),
  });
}
test('accepts complete stable and RC metadata with GitHub installer URLs', () => {
  validate(manifest());
  validate(manifest('v1.2.3'), 'v1.2.3');
});
test('rejects mismatched versions, missing platforms, and missing signatures', () => {
  const wrongVersion = manifest();
  wrongVersion.version = '9.0.0';
  assert.throws(() => validate(wrongVersion), /version mismatch/);
  const incomplete = manifest();
  delete incomplete.platforms['darwin-aarch64'];
  assert.throws(() => validate(incomplete), /Incomplete platforms/);
  const unsigned = manifest();
  unsigned.platforms['windows-x86_64'].signature = '';
  assert.throws(() => validate(unsigned), /Invalid updater metadata/);
});
test('rejects signature URLs and unexpected download origins', () => {
  const signatureUrl = manifest();
  signatureUrl.platforms['darwin-x86_64'].url += '.sig';
  assert.throws(() => validate(signatureUrl), /Invalid updater metadata/);
  const wrongOrigin = manifest();
  wrongOrigin.platforms['darwin-x86_64'].url = 'https://example.com/payload';
  assert.throws(() => validate(wrongOrigin), /Invalid updater metadata/);
});
test('automatically pushes draft metadata and promotes only stable releases', () => {
  const root = mkdtempSync(join(tmpdir(), 'oflh-updater-test-'));
  const remote = join(root, 'remote.git');
  const website = join(root, 'website');
  const file = join(root, 'latest.json');
  const git = (...args) => execFileSync('git', args, { stdio: 'pipe' });
  git('init', '--bare', '--initial-branch=main', remote);
  git('clone', remote, website);
  git('-C', website, 'config', 'user.name', 'github-actions[bot]');
  git('-C', website, 'config', 'user.email', '41898282+github-actions[bot]@users.noreply.github.com');
  writeFileSync(join(website, 'README.md'), 'Synthetic website fixture\n');
  git('-C', website, 'add', 'README.md');
  git('-C', website, 'commit', '-m', 'Initialize fixture');
  git('-C', website, 'push', 'origin', 'main');
  const bash = process.platform === 'win32' ? 'C:/Program Files/Git/bin/bash.exe' : 'bash';
  const sync = (tag, mode) => execFileSync(bash, [
    `${__dirname}/sync-updater.sh`.replaceAll('\\', '/'),
    website.replaceAll('\\', '/'), file.replaceAll('\\', '/'), tag, mode,
  ], { stdio: 'pipe' });
  writeFileSync(file, JSON.stringify(manifest()));
  sync('v1.2.3-rc.1', 'stage');
  assert.ok(existsSync(join(website, 'public/api/releases/v1.2.3-rc.1/latest.json')));
  assert.ok(!existsSync(join(website, 'public/api/latest.json')));
  const rebuilt = manifest();
  rebuilt.platforms['darwin-x86_64'].signature = 'rebuilt-signature';
  writeFileSync(file, JSON.stringify(rebuilt));
  sync('v1.2.3-rc.1', 'stage');
  const retried = git('--git-dir', remote, 'show', 'main:public/api/releases/v1.2.3-rc.1/latest.json').toString();
  assert.equal(JSON.parse(retried).platforms['darwin-x86_64'].signature, 'rebuilt-signature');
  assert.throws(() => sync('v1.2.3-rc.1', 'publish'));
  writeFileSync(file, JSON.stringify(manifest('v1.2.3')));
  sync('v1.2.3', 'stage');
  assert.ok(!existsSync(join(website, 'public/api/latest.json')));
  sync('v1.2.3', 'publish');
  const published = git('--git-dir', remote, 'show', 'main:public/api/latest.json').toString();
  assert.equal(JSON.parse(published).version, '1.2.3');
  sync('v1.2.3', 'publish');
  writeFileSync(file, JSON.stringify(manifest('v1.3.0-rc.1')));
  sync('v1.3.0-rc.1', 'stage');
  assert.equal(git('--git-dir', remote, 'show', 'main:public/api/latest.json').toString(), published);
  assert.throws(() => sync('v1.3.0-rc.1', 'publish'));
  assert.equal(git('--git-dir', remote, 'show', 'main:public/api/latest.json').toString(), published);
  assert.throws(() => sync('../invalid-tag', 'stage'));
});
