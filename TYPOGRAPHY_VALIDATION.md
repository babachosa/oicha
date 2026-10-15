# Vietnamese reading font and clean pages — 2026-10-15

Default manuscript typography is now Literata, a serif family designed for long-form reading with Vietnamese support. Four unmodified static faces (regular, italic, bold, bold italic) are embedded in the Rust executable and registered before creating a window. Startup reports font-registration failures instead of silently using a different reading font. Literata also replaces the previous Georgia accents in the application; the compact UI keeps Segoe UI. The font selector still offers Times New Roman and Segoe UI as alternatives.

Source: https://github.com/googlefonts/literata at pinned commit
`0c2761b727a1b3a7cffd313c37f0f5163dfc7a63`.
See `crates/oicha-app/assets/fonts/literata/README.md` for the four SHA-256 hashes.
Original SIL OFL 1.1 and copyright are included in the source and distribution.
No network requests or font installation are needed at runtime.

The page renderer now draws only paper surfaces and manuscript content. It no longer creates the chapter label/title or page-count footer. All sheets, including the first, start text at the same 76 px margin. The A4 proportions, fixed sheet heights, 24 px gaps and extra space after the manuscript remain. Chapter names remain in navigation/metadata; the document text is not edited. DOCX export retains its existing book/chapter structure and Times New Roman formatting.

Checks:

- Font cmap coverage: all four files contain all 145 tested codepoints (Vietnamese precomposed letters, NFD constituent letters/combining marks and literary punctuation). No missing mappings. Evidence: `artifacts/literata-font-coverage.json`.
- Native Windows text shaping: four distinct Literata faces, Vietnamese precomposed characters and combining sequences at 16/20/28 px, all runs use the requested face without fallback. This check is part of both native smoke scenarios.
- 46 workspace tests, format and Clippy with warnings denied pass. Core commands, file format, Cargo dependencies and lockfile are unchanged.
- Native debug: `artifacts/literata-debug/results.json`. Includes baseline/IME/undo, wrapping, page-boundary navigation, page margins, scroll motion, Enter/soft-wrap follow, saving and load/close regressions.
- Visual inspection: isolated own-window captures `artifacts/literata-visual/top.png` and `pages.png`, showing Vietnamese text, the clean first sheet and the undecorated page boundary.

Release evidence and binary hash are recorded in VALIDATION.md.
