# Audit fixes — 2026-10-17

Candidate: Oicha 0.2.4, Windows x64, Rust 1.98.1 MSVC, GPUI 0.2.2.
This addresses the three findings in [the 0.2.3 audit](AUDIT_REVIEW_2026-10-17.md).

## Changes

- History restore exits Visual/Visual Line, restores valid offsets and clears the
  previous vertical goal. Direct body edits also end Visual selection before it
  can refer to the previous revision. Visual rendering additionally confines its
  endpoints to valid grapheme boundaries. GUI paste replaces the full painted
  selection; Select All leaves the previous Visual selection state.
- Cut verifies that clipboard text matches the selection before deleting it.
  If Windows refuses clipboard access, the manuscript stays unchanged and a
  localized retry notice appears. The notice does not block autosave. Native
  probes assert that a failed clipboard copy preserves body, selection and
  generation. Clipboard QA retries only no-op failures within a bounded window.
- GUI Home/End use shaped display rows in Normal, Insert and Vim-off modes.
  Shift+Home/End selects within the row. A soft-wrap endpoint retains trailing
  affinity, so repeated End stays on the same row. Explicit Vim logical motions
  retain their documented behavior.
- DOCX's ZIP dependency moves from 4.6.1 to 9.0.4. No advisory exception is added.
  The new lockfile removes derive_arbitrary, adds typed-path 0.12.3 and retains
  the reviewed grid backport unchanged.

## Evidence

Core regression tests exercise Visual and Visual Line in both directions,
Unicode/emoji/combining marks, undo to an empty body, redo, Cut/Paste, Select All,
stale offsets and GUI row boundaries/selection/affinity. The unchanged standalone
audit probe now reports Normal mode and no selection panic for all three actions:
`artifacts/fix-024-core-probe.json`.

The native smoke also exercises 16 GUI dispatch cases (Ctrl+Z, toolbar Undo,
Ctrl+X, Ctrl+V, both Visual modes and selection directions), Undo/Redo round trips,
Shift+Home/End and repeated End with Vim Insert and Vim disabled. It preserves
the clipboard around these additional checks. `tools/native-editing-smoke.ps1`
sends real Windows key/character messages to isolated owner-checked windows for
the original crash sequence and Home/End cases. The user's app/manuscripts are
not used as fixtures.

The dependency scan checks all 741 registry package/version entries, verifies
the grid source hash, and reports zero blocking advisories. The existing seven
INFO Unmaintained exceptions remain explicitly reported (10 package/version
matches): `artifacts/fix-024-dependencies.json`.

Release artifacts:

- `dist/Oicha.exe`: SHA-256
  `DCF48FCFD42FD406CD4824E0122D936900B750876784C5B7728B2FCB8097AE80`.
- `dist/Oicha-0.2.4-Windows-x64-Setup.exe`: 4,747,765 bytes, SHA-256
  `89771D7EFE0FBE5E0ABC227F9911879FF0867318C3ED7B8A75ED6DB85EC11BC8`.
- `dist/oicha-update.json`: signed with the existing Ed25519 release key.
- `dist/Oicha-Windows-x64.zip`: the same packaged application and runtime files.

The tested release executable and packaged executable have identical SHA-256.
Packaging record: `artifacts/fix-024-package-stable.json`.

## Final release verification

| Check | Result |
| --- | --- |
| Workspace tests | PASS: 62 tests |
| Real Vim reference | PASS |
| Format and Clippy, all targets, warnings denied | PASS |
| Release security regressions | PASS: 2 tests |
| OSV dependency gate | PASS: 741 versions, zero blocking advisories |
| Release build and NSIS packaging | PASS |
| Native full suite, load success/failure and queued close | PASS both |
| Native GUI Visual/Visual Line history and clipboard matrix | PASS: 16 cases per scenario, including failed-copy no-delete guard |
| Native Home/End/Shift selection, repeated End | PASS with Vim Insert and Vim off |
| Real Windows key reproduction: Visual undo and Home/End | PASS: 3 cases |
| Existing vertical cursor matrix | PASS: 7 cases |
| Scroll edges and first-row reveal | PASS: 64 boundary samples |
| Caption controls with popup and persisted close | PASS |
| Native DOCX exports, independent ZIP/XML reader | PASS: both files have 5 well-formed XML parts |

Final evidence uses `artifacts/fix-024-stable-native/`, `...-stable-editing/`,
`...-stable-cursor/`, `...-stable-scroll/`, `...-stable-caption/` and
`artifacts/fix-024-stable-docx.json`. All native fixture processes exited; the
user's installed app remained running. Earlier failed-candidate directories are
retained for the timing/clipboard investigation below, not counted as passes.

## Limits

Installer/updater implementation is unchanged. The installed Oicha is running,
so this pass does not reinstall/uninstall it or perform a real restart-upgrade.
It uses isolated native manuscripts and the public feed download/signature/hash
probe. The prior 0.2.1 → 0.2.2 full upgrade evidence remains historical. No claim
is made about all third-party IMEs, power-loss fault injection, long-term soak
testing, Word round trips, accessibility or platforms other than Windows. The
installer remains unsigned with Authenticode by the user's existing choice;
update metadata uses the existing Ed25519 key.

## Animation check timing

An initial release smoke run passed the load-success scenario but its load-failure
scenario failed the existing Vim-o viewport assertion at a fixed 150ms wait.
The diagnostic file is retained in `artifacts/fix-024-native/`. Re-running the
same executable without source changes passed both scenarios and all native
editing/cursor/scroll/caption checks (`artifacts/fix-024-native-recheck/`).
The QA code now samples geometry/animation completion every 25ms, up to one second,
instead of relying on a 150ms sleep for this assertion. The expected 32% writing
area and two-pixel tolerance are unchanged. The final run records its measured
frames in `*.smoke-result.vim-o-motion.json`; no production scrolling algorithm
was changed to address this test timing issue.

Another candidate check caught the same fixed-wait problem at `zz`: painted
bounds still differed from the current scroll offset by about 15 pixels. All
settled-motion checks in the editor geometry section now wait for two consecutive
samples with no pending caret/scroll/input motion and painted bounds matching
the scroll offset. Polling is 25ms with a two-second bound. Existing intermediate
animation checks and final position assertions remain unchanged. The complete
debug smoke passes both load/close scenarios with this helper:
`artifacts/fix-024-stable-debug-2/`.

A subsequent final-candidate run encountered Windows `OpenClipboard` access
denied during the GUI paste fixture (recorded in
`artifacts/fix-024-native-final/load-success.stderr.txt`). The product now verifies
GUI Cut clipboard writes before deletion. Clipboard QA waits/retries when access
is temporarily unavailable and never repeats a successful mutation. This handles
actual clipboard contention without weakening the expected document assertions.
The smoke runner also detects explicit failure reports early and closes only its
own fixture instead of waiting for the full timeout.
