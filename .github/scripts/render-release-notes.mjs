import { readFileSync } from 'node:fs';

const tag = process.argv[2];
if (
  process.argv.length !== 3 ||
  !/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(tag ?? '')
) {
  throw new Error('Usage: node render-release-notes.mjs <version tag, e.g. v1.2.3>');
}

const template = readFileSync(
  new URL('../release-notes/downloads.md', import.meta.url),
  'utf8',
);
if (!template.includes('{{TAG}}')) {
  throw new Error('Release notes template has no version placeholders');
}
process.stdout.write(template.replaceAll('{{TAG}}', encodeURIComponent(tag)));
