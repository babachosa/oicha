# Display-row dd — 2026-10-19

Local, unpublished change after Oicha 0.2.7.

The user requested two deliberate writing-oriented deviations from stock Vim:
delete only the displayed row, and retain a final empty writing row instead of
moving onto the preceding line. Other logical-line operators are unchanged.

## Implementation

- `dd` uses the renderer's shaped display-row ranges. `3dd`, `d3d` and multiplied
  counts such as `2d2d` count those rows. Before the first layout, logical lines
  provide the fallback ranges.
- Whole paragraph deletion consumes its following separator. Deleting only a
  paragraph's last wrapped row preserves the separator before an untouched next
  paragraph, and places the caret on that next row.
- Deleting a final logical line retains the preceding newline. Deleting a final
  soft-wrapped row creates one hard newline after its surviving prefix so that
  the empty writing row remains. Repeated `dd` on that empty final row is a no-op.
- Normal mode and the existing linewise paste-register behavior are retained;
  `i` starts writing in the retained final row without Enter. Undo/redo restores
  the complete edit, styles and cursor. Paragraph text outside the selected rows
  is preserved; middle-paragraph text reflows normally after deletion.
- `yy`, `cc`, `dj/dk`, Visual Line and other linewise operations keep their
  logical-line semantics. Help and README explicitly describe the custom `dd`.

## Verification

- Workspace tests: PASS, 73 tests. Four new test groups cover first/middle/last
  wrapped rows, paragraph boundaries, empty rows, EOF, counts, Unicode combining
  marks and emoji, styles, register/paste behavior, one-step undo/redo and typing
  directly after deletion.
- Installed Vim reference: PASS, 103 remaining stock-compatible cases. The old
  stock `dd`-at-EOF case was moved to custom behavior regressions, since its
  expected behavior intentionally changed at the user's request.
- Clippy with warnings denied and format checks: PASS.
- Optimized release build: PASS. This local binary still carries version 0.2.7;
  it is not a new release or installer.
- Native debug app: PASS for both load-success and load-failure scenarios,
  saved concurrent edits, queued-close markers and exit 0.
- New native checks use actual shaped Vietnamese rows. They delete only the
  second row of a long paragraph, verify the exact removed byte range/register,
  and undo in one step. For both the user's two-line example and a long wrapped
  final paragraph, they verify that the retained empty row has the same renderer
  Y coordinate, then insert Vietnamese immediately using `i` without Enter.
- Evidence: `artifacts/dd-visual-2026-10-19/results.json`. The previously observed
  nonfatal GPUI `window not found` message during guarded close is retained in
  stderr; exit codes, saved files and close markers are checked independently.

The installed application and real manuscripts were not changed. No tag,
installer or GitHub release was created for this local change.
