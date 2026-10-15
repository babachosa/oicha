# Whole-word prose wrapping and integrated titlebar — 2026-10-15

GPUI 0.2.2's default wrapping classifies only selected Unicode ranges as word characters; Vietnamese Latin Extended Additional letters such as `ề` were treated as break opportunities. This could render `ni` at the end of one row and `ềm` at the start of the next.

Oicha now shapes each paragraph once and computes its own prose wrap boundaries at breakable whitespace, preserving punctuation with its token. Oversized tokens fall back to shaped grapheme/cluster boundaries. NBSP and narrow NBSP are kept inside their token. Trailing whitespace is retained; no line breaks are inserted into document text. The same WrappedLineLayout boundaries drive glyph painting, hit testing, selection, IME coordinates, pagination and Vim display-row navigation. Cached paragraph metadata and shaped glyph runs remain shared; no GPUI source patch or dependency change is involved.

The Windows system caption is hidden using GPUI's transparent titlebar option. A native GPUI titlebar draws the book title, dirty indicator and window controls. Windows hit areas handle dragging, double-click, resizing and Min/Max, including restore. Close is handled once by the existing application save/queued-I/O guard. The top/side insets leave resize edges available. Controls use vector paths rather than font symbols. Modal backdrops begin below the caption, so the window controls remain usable while a popup is open.

Verification:

- 49 workspace tests pass: the existing 46 plus 3 prose-wrap tests covering NFC/NFD Vietnamese, punctuation, oversized words, emoji/combining clusters, whitespace, empty text and nonbreaking spaces.
- Format and Clippy with warnings denied pass.
- Native wrap regression shapes the user's `niềm` example in NFC and NFD, mixed font styles, regular/italic/bold: a width that fits only part of the word must move the whole word to the next row.
- Native debug success/failure scenarios: `artifacts/prose-titlebar-debug-4/results.json`. Existing IME/baseline/undo, Vim A/0/j/k, pagination/hit testing, scrolling, Enter and load/close checks pass.
- Real Windows nonclient-event test: `tools/native-window-smoke.ps1`, evidence `artifacts/prose-titlebar-window-debug/results.json`. Checks Drag/TopResize/Min/Max/Close hit results; sends caption down/up messages to the verified fixture HWND; asserts maximize then restore, minimize, and successful close with edited text persisted. The driver touches only its own isolated fixture process/window. It does not move the user's desktop pointer.
- The same caption test with an open Help popup passes at `artifacts/prose-titlebar-window-modal-debug/results.json`.
- Visual inspection of the exact italic Vietnamese example and integrated caption: `artifacts/prose-titlebar-visual/prose.png`.
- The second-Enter check now accounts for page-margin gaps and the measured viewport: a new sheet can move the next row past the writing-area threshold in a shorter window. Ordinary Enter with ample space must still preserve the offset exactly.

Release evidence and executable hash are recorded in VALIDATION.md. Native hit-test coverage establishes the drag/resize regions; physical multi-monitor dragging and Windows snap-menu variants have not been exhaustively exercised.
