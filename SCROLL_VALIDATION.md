# zz and end scroll space — 2026-10-14

The 2026-10-15 display-page and soft-wrap update supersedes the single-sheet surface described here; see PAGINATION_VALIDATION.md.

- `zz` in Normal/Visual centers the current shaped display row, preserving cursor column, selection and manuscript/history. Near the document start, scrolling clamps at the top. Insert mode continues to type ordinary z characters.
- `[count]zz` first moves to the requested logical line, retaining the actual column (bounded by that line). Escape cancels the prefix; unsupported suffixes/operators do not edit text. The first z does not reveal an offscreen cursor before the second z arrives.
- A separate, non-shrinking scroll spacer adds 65% of the measured editor viewport height below the paper. It updates on resize, exists with Vim enabled or disabled, and is excluded from text, undo, word counts and exports. Compact popup inputs have no spacer.
- Paper width is resolved before intrinsic height measurement. This corrects the prior case where the flex basis used wider wrapping than the final painted text, underestimated the paper height and clamped scrolling too early after reflow.

Validation:

- Workspace: 38 core, 1 glyph-layout and 2 security tests pass (41 total).
- Vim oracle: 67 cases pass, including zz column preservation, count, short line, out-of-range count, Visual selection, Escape and unsupported suffix.
- Clippy with warnings denied and format checks pass.
- Both native scenarios pass on debug and release. The new assertions center the final wrapped row at 28 px, scroll past it without text changes, keep the first z stationary, center again, resize to 1040 × 660, check centering and resized spacer height, and restore the original size. Existing input/IME/undo/Vim/modal/save/load/close/export regressions remain exercised.

Evidence: artifacts/zz-debug-final/results.json and artifacts/zz-release/results.json. An isolated native preview was captured at artifacts/zz-visual/scroll.png. See VALIDATION.md for the current executable hash and the broader verification limits.

## Enter at EOF — 2026-10-14

A paragraph break at EOF now requests a fresh writing area. If the new caret row is in the lower 35% of the viewport, scroll it to 32% of the viewport height. If enough room remains, retain the viewport; breaks inside existing text use normal cursor reveal. This also applies to Vim o on the final logical line and a single newline supplied by the native input adapter. Compact popup fields are excluded.

The request survives ordinary text/IME input arriving before the next frame and is consumed once. Native/toolbar undo and redo cancel a pending request. The scroll uses the measured display-row position and clamps to the existing extra-space range, adding no text beyond the actual paragraph break.

Native checks: Vim ON/OFF EOF Enter plus immediate Vietnamese composition/commit; a second Enter without a repeated viewport jump; newline undo; Enter near the start and near the lower edge inside existing text; 28 px text at 1040 × 660; and Vim o. Existing zz, viewport resize, IME, history, modal, save/load/close and export checks remain enabled. Workspace 41 tests, formatting and Clippy pass.

Evidence: artifacts/enter-page-debug-verified/results.json and artifacts/enter-page-release/results.json. Visual preview: artifacts/enter-page-visual/page.png. A preliminary artifact named enter-page-debug-final used the prior debug executable after a compile failure and is excluded from validation; debug-verified was built successfully before execution.
