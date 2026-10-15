# Popup update — 2026-10-14

Native Rust/GPUI only. Input dialogs, help, chapter deletion and extension-converted export-overwrite confirmation share a modal shell. Windows file pickers remain native.

- Opening: 200 ms fade and 14 px upward slide with quintic easing. Closing: 120 ms fade/slide, starting from the current opening progress when interrupted.
- Cream/sage palette, 20 px card corners, layered soft shadows, clear labels/descriptions, separated input and action areas, button hover/pressed states.
- Help content scrolls in a bounded region so its close button remains visible at the minimum 1040 × 660 client size.
- Each transition uses a fresh animation ID. Delayed close callbacks check the transition generation before dropping content or returning focus.
- Modal focus blocks document edits, including throughout fade-out. Input keyboard shortcuts stay with the input. Ctrl+Enter submits multiline notes. Confirmation defaults to cancellation; Tab chooses the other action and Enter applies it. Repeated submission is ignored while closing.

Validation: workspace tests (39) and Clippy pass. Native smoke scenarios exercise search cancel, help input isolation, closing then opening another dialog before the close callback, input focus after reopening, document input blocked during fade-out, default confirmation cancel, explicit chapter deletion applied once, and multiline notes submitted once. They also retain the existing IME, Vim navigation, undo, layout, save/load/close and DOCX regressions.

Visual inspection: isolated native debug windows captured with PrintWindow, never the desktop. Search, help and chapter confirmation checked; help and confirmation checked at minimum window size. Evidence: artifacts/popup-visual/search.png, help-small.png, delete-small.png. Screenshots precede the final help footer hint addition.

Release smoke evidence: artifacts/popup-release/results.json. Both scenarios exit 0 and verify queued-close markers and persisted edits. GPUI emits one “window not found” log in each scenario at the final window-close timestamp; the full assertions and shutdown markers pass. This is local verification of the popup update, not certification of accessibility, every Windows input method or every hardware configuration.
