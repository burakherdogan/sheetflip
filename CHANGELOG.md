# Changelog

## 0.1.0

Initial release.

### Added

- A single executable with an embedded English/Turkish web interface, light/dark themes and PDF.js previews.
- CUPS printing on Linux and macOS: single-sided, manual double-sided and A5 booklets.
- Arbitrary 1–100 pages per face for single-sided and double-sided jobs, with A4 multi-page layouts.
- Front/back sheet previews and a popup viewer with zoom and sheet navigation.
- Page selection, copies, print quality, grayscale, page numbers and flip-edge selection.
- A two-sheet printer calibration flow with saved per-printer back-face order and rotation.
- Release build targets for Linux x86_64/ARM64 and macOS Intel/Apple silicon.

### Known limitations

- No Windows backend, authentication or built-in TLS. The default listener is network-accessible.
- Physical printer testing is limited; calibrate and test your own printer before a large job.
- Annotations and form widgets are not flattened for multi-page or booklet output.
- Preview is limited to the first 24 sheets and is not a physical-output guarantee.

See the [README](README.md) for requirements, safe startup and other limits.
