# Fixed display pages and soft-wrap follow — 2026-10-15

The writing surface is now a stack of A4-proportioned sheets instead of a single growing rectangle. At 760 px width, a sheet is 1075 px tall. Resizing changes the width/height together and reflows text. At a given width, typing keeps each sheet's height constant; a new sheet is allocated only when another display row is required. Margins are 76 px, the inter-page gap is 24 px, the first sheet reserves a fixed chapter-heading band, and footers number the sheets within the active chapter.

Hard paragraph breaks, empty paragraphs and soft wraps use the same pagination geometry. There are no synthetic characters, whitespace or manual page-break commands in the stored manuscript. DOCX export retains its documented Times New Roman 12 pt/A4 settings; the on-screen typography remains a reading/writing preference, so this is not a Microsoft Word print-preview or page-count parity claim.

At EOF, typing onto a new shaped display row now requests the same writing area as Enter. If the new row reaches the lower 35% of the viewport, it moves to 32% of the viewport height. In-row typing/IME updates and navigation do not request a new writing area. Existing Enter, Vim o and zz behavior remains supported.

The layout records each row's page position, while Vim navigation still counts only text rows. Hit testing chooses the nearest text row through page margins/gaps and returns original UTF-8 byte offsets. Mouse selection keeps trailing affinity at a wrapped row end. IME candidate rectangles use the same page-aware cursor positions.

Rendering retains GPUI's shaped font/glyph IDs and indexes glyphs into cached rows. Only visible rows and sheets are painted; a long paragraph spanning many sheets is not scanned or reshaped once per sheet. Baselines remain fixed to the base font, including Vietnamese fallback accents, and emoji/bold/italic/underline remain supported.

Validation:

- 43 workspace tests: 38 core, 2 pagination, 1 glyph-index and 2 security regressions.
- Format and Clippy with warnings denied pass.
- Native debug and release exercise automatic soft-wrap follow with Vim ON/OFF, fixed sheet height while typing, no inserted newline from soft wrapping, margin/row hit testing for the full fixture, j crossing a page boundary using original byte positions, zz, resize, Enter/o, immediate Vietnamese composition, history, modal lifecycle, save/load/close and DOCX export.
- Visual review: chapter heading and Vietnamese paragraphs; adjacent sheets, margins, footer numbers, italic text and color emoji. Captures use only isolated application windows. Evidence: artifacts/pages-visual/top.png and pages.png.

Native debug evidence: artifacts/pages-debug-4/results.json. The new cross-page check waits for the undo frame before inspecting committed geometry. Native release evidence: artifacts/pages-release/results.json. Earlier failed intermediate runs are excluded. Remaining application verification limits are listed in VALIDATION.md.
