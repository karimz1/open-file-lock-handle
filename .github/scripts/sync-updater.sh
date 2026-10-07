#!/usr/bin/env bash
set -euo pipefail
website="$1"
manifest="$2"
tag="$3"
mode="$4"
[[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]]
[[ "$mode" == stage || "$mode" == publish ]]
# RCs remain versioned and never replace the stable application endpoint.
if [[ "$mode" == publish ]]; then
  [[ "$tag" != *-* ]]
fi
node - "$manifest" "$tag" <<'NODE'
const fs = require('node:fs');
const [file, tag] = process.argv.slice(2);
const manifest = JSON.parse(fs.readFileSync(file, 'utf8'));
if (manifest.version !== tag.slice(1)) throw new Error('Manifest version mismatch');
const expected = {
  'darwin-x86_64': 'oflh-desktop.darwin.amd64.app.tar.gz',
  'darwin-aarch64': 'oflh-desktop.darwin.arm64.app.tar.gz',
  'windows-x86_64': 'oflh-desktop.windows.amd64-installer.exe',
  'windows-aarch64': 'oflh-desktop.windows.arm64-installer.exe',
};
if (Object.keys(manifest.platforms).length !== 4) throw new Error('Incomplete platforms');
for (const [platform, filename] of Object.entries(expected)) {
  const entry = manifest.platforms[platform];
  const url = `https://github.com/karimz1/open-file-lock-handle/releases/download/${tag}/${filename}`;
  if (entry?.url !== url || typeof entry.signature !== 'string' || !entry.signature.trim()) {
    throw new Error(`Invalid updater metadata for ${platform}`);
  }
}
NODE
destination="$website/public/api/releases/$tag/latest.json"
mkdir -p "$(dirname "$destination")"
# Stage uses draft metadata; publish uses the validated published release asset.
cp "$manifest" "$destination"
if [[ "$mode" == publish ]]; then
  cp "$manifest" "$website/public/api/latest.json"
fi
git -C "$website" config user.name 'github-actions[bot]'
git -C "$website" config user.email '41898282+github-actions[bot]@users.noreply.github.com'
git -C "$website" add public/api
if ! git -C "$website" diff --cached --quiet; then
  git -C "$website" commit -m "Update OFLH $tag updater metadata ($mode)"
  git -C "$website" push origin HEAD:main
fi
