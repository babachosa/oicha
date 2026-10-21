# Vim character replacement — 2026-10-21

Local, unpublished change after 0.3.0. `r{character}` now replaces the character under the caret and stays in Normal mode. Counts such as `3ra` replace exactly that many graphemes in the logical line; insufficient remaining characters leave the text and cursor unchanged. Replacement can span soft-wrapped display rows but cannot consume a paragraph separator in Normal mode.

Visual/Visual Line replacement covers the selected characters while retaining paragraph separators. Empty visual rows exit to Normal without a phantom undo entry. Unicode graphemes, including Vietnamese combining marks and emoji, stay intact. Each replacement is one undo transaction, retains per-position formatting and does not change the register/clipboard. Counted redo returns to the start of the change, matching Vim.

Normal `r` + Enter replaces the counted span with one newline; Tab inserts a literal tab. Visual `r` + Enter inserts literal CR characters as Vim does, preserving existing LF separators. Escape cancels the pending command. GUI shortcuts, focus changes and mouse repositioning cancel the pending replacement so subsequent input cannot unexpectedly replace a character. Replacement targets such as `/`, `:`, `n`, `o`, digits and `r` are treated literally. `ro`/`rO` do not trigger the open-row writing-area scroll request.

F1 help and the README list the command in Vietnamese and English. As with the existing scoped Vim engine, this does not add full Replace mode (`R`), dot-repeat, control-character quoting, digraph input or the Ctrl+E/Y replacement variants.

## Checks

- Final workspace tests: **93 passed**, two optional diagnostics ignored. Six new editor regressions cover counts/literals/Unicode, cancellation/failure/modes, styles/register/history, Visual direction/paragraphs, Enter/Tab, and empty visual rows. `artifacts/vim-r-workspace-tests.txt`.
- Explicit comparison against installed Git Vim: passed, including 29 new replacement cases covering cursor position, logical-line boundaries, Enter, Visual, Unicode and undo/redo. `artifacts/vim-r-oracle-tests.txt`. The oracle exposed redo-caret and Visual-Enter semantics; both were corrected to match Vim.
- Clippy with warnings denied and Rustfmt: passed. The existing upstream proc-macro-error2 future-compatibility notice remains.
- Debug native renderer: both load-success and load-failure scenarios passed. The new adapter checks dispatch 13 replacement targets, counts, undo/redo, Escape and GUI-shortcut cancellation, asserting that targets cannot open Ex/search prompts or request open-row scrolling. `artifacts/vim-r-debug-2026-10-21/results.json` and each scenario's `*.smoke-result.replace.json`. This run preceded the final empty-row no-op fix; the final-source unit tests cover that fix.

## Final optimized EXE

- `cargo build --release -p oicha --locked`: passed. `target/release/oicha.exe`, 16,990,720 bytes; SHA-256 `80F3E483420A0D329E2FD41F80BE81DD793DB119440D79DFFCAF55774DA287BD`.
- Both final-source optimized native scenarios passed, including all 13 replacement targets and broader IME/history/formatting/wrapping/scroll/modal/save/export/concurrent-load/queued-close regressions. `artifacts/vim-r-release-final-2026-10-21/results.json` and each `*.smoke-result.replace.json`.
- Acceptance record: `artifacts/vim-r-acceptance-2026-10-21.json`. `git diff --check` passed.

The preceding 0.3.0 binary is preserved at `target/release/oicha-before-vim-r-2026-10-21.exe`. The installed user app and its manuscript remain open and untouched. No release, installer or push is performed by this task.
