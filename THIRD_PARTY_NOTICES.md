# Third-party notices

## PDF.js 4.10.38

Sheetflip embeds the following JavaScript files for browser-side PDF previews:

- `src/vendor/pdf.mjs`
- `src/vendor/pdf.worker.mjs`

Copyright 2024 Mozilla Foundation.

Licensed under the Apache License, Version 2.0. The full license is included in
[`licenses/PDF.js-LICENSE`](licenses/PDF.js-LICENSE). The original copyright and
license headers remain in both files.

Upstream: <https://github.com/mozilla/pdf.js>

Pinned source release: <https://github.com/mozilla/pdf.js/tree/v4.10.38>

The bundled files are unmodified copies of `build/pdf.min.mjs` and
`build/pdf.worker.min.mjs` from the official `pdfjs-dist` npm package, version
4.10.38, renamed locally. Both were checked byte-for-byte against:
<https://registry.npmjs.org/pdfjs-dist/-/pdfjs-dist-4.10.38.tgz>.

The license text was obtained from the pinned upstream release:
<https://raw.githubusercontent.com/mozilla/pdf.js/v4.10.38/LICENSE>.

The distribution package has no separate top-level NOTICE file. Its optional
CMaps and standard-font asset directories are not bundled by Sheetflip.

## Rust dependencies

Sheetflip also uses third-party Rust crates listed in `Cargo.toml` and pinned in
`Cargo.lock`. Those components retain their own licenses; Sheetflip's MIT
license does not replace them. This document's PDF.js attribution is not an
exhaustive inventory of Rust dependency licenses.
