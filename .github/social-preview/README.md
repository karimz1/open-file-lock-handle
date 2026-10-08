# Social preview

Builds [`.github/social-preview.png`](../social-preview.png) (1280 × 640). GitHub
uses that image for release cards and link previews once it's uploaded under
Settings → General → Social preview.

| File | What it is |
| --- | --- |
| `template.html` | The card. Colours match the site (`#101113`, `#4264db`, `#98b0f3`). |
| `Inter-subset.woff2` | Inter, Latin subset (SIL OFL 1.1). Keeps the letters identical everywhere. |
| `terminal-ancestry.png` | Process details panel from the [asciinema recording](https://asciinema.org/a/1266562). |
| `render.cjs` | Playwright script. Writes `../social-preview.png`. |

The desktop shot and app icon come from `images/desktop.png` and
`crates/oflh-desktop/app-icon.svg`, so the card updates when those change.

## Build

Needs Node and Playwright's Chromium:

```sh
npm i -D playwright && npx playwright install chromium
node .github/social-preview/render.cjs
```

Renders at exactly 1280 × 640 (`deviceScaleFactor: 1`). Keep text inside a 40 px
margin; GitHub crops and scales the card.

CI ([`social-preview.yml`](../workflows/social-preview.yml)) re-renders the card
when any of these files change and fails if an image or the font doesn't load or
the PNG isn't 1280 × 640. It doesn't diff against the committed PNG; the render is
attached as the `social-preview` artifact.

## Upload

GitHub does not pick this file up from the repo. After regenerating, upload it
again under Settings → General → Social preview → Edit.
