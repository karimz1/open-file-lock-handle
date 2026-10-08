# Social preview

Source for `.github/social-preview.png`, the 1280×640 card GitHub shows when the
repository is linked on social sites and chat apps.

- `template.html` – the card layout. Colors follow the website's dark theme
  (`--bg #101113`, `--accent #4264db`, `--accent-light #98b0f3`); the wordmark
  matches the site's `oflh.` in Inter 750.
- `Inter-subset.woff2` – Inter (SIL Open Font License 1.1), subset to Latin so
  the card renders the same on every machine without installing fonts.
- `terminal-ancestry.png` – the process-tree panel, cropped from frame 20 of
  `images/demo.gif`.
- The desktop screenshot and app icon are read straight from `images/desktop.png`
  and `crates/oflh-desktop/app-icon.svg`, so the card follows them when they change.

## Regenerate

Needs Node and the `playwright` package with its Chromium build.

```sh
npm i -D playwright && npx playwright install chromium   # once, anywhere
node .github/social-preview/render.cjs                   # writes .github/social-preview.png
```

The script renders at exactly 1280×640 with `deviceScaleFactor: 1`. Keep text
inside a 40px margin; GitHub and link previews crop and scale the card.

## Upload

GitHub does not read this file automatically. Upload it once under
**Settings → General → Social preview → Edit → Upload an image**, and again
after regenerating it.
