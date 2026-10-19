# Project menu and compact manuscript sidebar — 2026-10-19

Local development update after the single-file library change; no public release or push.

- Each Home project card has a dedicated ellipsis button. Its click does not open the project. The menu is rendered above the scrolling list, snaps inside the window and dismisses on outside click, scrolling or Escape.
- Delete project opens the existing animated confirmation modal with the selected project's title. The default Enter action cancels; deliberate Tab/Enter or the destructive button confirms. The menu and confirmation support Vietnamese and English.
- The database removes the manuscript, index and internal checkpoint in one redb transaction, updates the persisted active-project reference and never reuses deleted IDs. A failed transaction preserves the current UI and document.
- Deleting the current project clears its live editors/subscriptions and autosave eligibility. Deleting the final project leaves a valid empty `.oicha` library; manual save and the next autosave interval do not recreate it. Deleting another project preserves the live manuscript and cursor/editor state.
- Per the user's clarification, this layout change targets the top of the left MANUSCRIPT sidebar. Its width is now 236px, matching the right inspector (formerly 248px). Header vertical padding is 12px (formerly 18px); the title is 15px (formerly 17px), with tighter spacing. The formatting toolbar is unchanged.

## Checks

- Workspace tests: 77 passed; two optional diagnostics ignored by default. Two additional core tests cover inactive/current/final deletion, checkpoint/index cleanup, rejecting a missing ID, reopening, monotonic IDs and one-file persistence.
- Clippy with `-D warnings` and rustfmt passed.
- Debug native fixture: `artifacts/project-actions-debug-2026-10-19/results.json`. Menu Escape/Enter, default cancel, duplicate confirm, deleting inactive/current/final projects, failed delete preservation and waiting beyond the autosave interval all passed. An independent closed-file inspection reopened the library and found zero projects and no sidecar files.
- Computer Use checked an isolated 1040×660 native window: clicked the ellipsis, clicked Delete to open the correctly named confirmation, cancelled with Escape, and opened the preserved project to inspect the compact left sidebar. The popup extends below the card without being clipped. Actual destructive confirmation was exercised only in the owned CLI fixture, not on a user's manuscript.
- The two existing user windows were left running. The previous `target/release/oicha.exe` was preserved as `oicha-before-project-actions-2026-10-19.exe` before compiling its replacement, so the running test copy was not terminated.

## Final optimized EXE

- `target/release/oicha.exe`: 16,806,400 bytes.
- SHA-256: `4281677792248A36F11B76A20DE3B3A5CBCCDFF3F436DF61E226C6CAFBF576BD`.
- Native project actions and closed-file empty-library inspection: `artifacts/project-actions-release-2026-10-19/results.json` — passed.
- Two-project Home/save/switch regression: `artifacts/project-actions-library-release-2026-10-19/results.json` — passed.
- Both complete native renderer/load/queued-close scenarios: `artifacts/project-actions-full-release-2026-10-19/results.json` — passed, including real wrapping/cursor/page behavior with the narrower left sidebar.
- Combined binary fingerprint and acceptance: `artifacts/project-actions-release-2026-10-19/acceptance.json`.

The existing GPUI `window not found` diagnostic can appear after an isolated async fixture removes its window; the drivers independently require exit 0, PASS reports and successful closed-file inspection. The upstream `proc-macro-error2` future-compatibility warning remains. No installer or release feed was published.
