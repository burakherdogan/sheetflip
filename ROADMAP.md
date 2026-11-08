# Roadmap

Planned work for Sheetflip, roughly in priority order. Nothing here is promised for a specific release.

## Next

- [ ] **Windows support.** A printing backend that does not depend on CUPS `lp`/`lpstat`, plus Windows release builds.
- [ ] **Physical print checks.** Verify the printer test, booklets, short-edge flipping and multi-page layouts on real printers, and record which printers have been confirmed.
- [ ] **Optional access protection.** A password or token for when Sheetflip is reachable from the network.
- [ ] **Flatten annotations and form fields** in multi-page and booklet output, so filled forms print completely.

## Later

- [ ] **Easier installation:** `cargo install sheetflip` (crates.io), a Homebrew formula, and AUR / `.deb` packages.
- [ ] **Paper sizes other than A4** for multi-page and booklet layouts (Letter, A3).
- [ ] **Signed and notarized macOS builds.**
- [ ] **Printer job status:** show when CUPS has finished a side instead of asking the user to wait.
- [ ] **Robustness:** atomic settings writes, server-side duplicate-submit protection, random job IDs and PDF parsing off the async runtime.

## Done

- [x] Single-sided, manual double-sided and A5 booklet printing through CUPS.
- [x] 1 to 100 pages per side, with the same layout in the preview and the printed PDF.
- [x] Per-printer test that saves back-side order and rotation.
- [x] Sheet preview popup with front and back faces and page labels.
- [x] Linux and macOS release builds with checksums.

Suggestions and printer reports are welcome in [issues](https://github.com/burakherdogan/sheetflip/issues).
