# Serve updater checks from the OFLH website

Move the Tauri manifest to `https://oflh.karimzouine.com/api/latest.json`
so automatic update checks do not increase GitHub release asset download counts.
Keep signed installer payloads and detached `.sig` files as release assets.

Release assembly must commit each manifest to the website's versioned static
directory. Only published stable releases may replace `/api/latest.json`;
RCs and drafts must not change stable updates. Seed the website with the existing
v0.6.0 manifest and document the cross-repository automation credential.

GitHub issue creation was attempted but the connected integration returned
HTTP 403 (`Resource not accessible by integration`). This file preserves the
ticket for later issue creation.
