# Language preference persistence — Oicha 0.2.7

## Reproduction and cause

The earlier 0.2.6 native failure left English persisted after requesting
Vietnamese. An unchanged-binary retry passed, but did not fix the problem.

This investigation reproduced the underlying persistence failure before the fix:

- A real Windows file handle granting read/write sharing but not delete sharing
  blocked atomic replacement. The save failed immediately with OS error 5,
  `Access is denied`, even though the handle was released 80 ms later.
- Repeated preference writes to a fresh isolated folder inside this OneDrive
  workspace failed at zero-based iteration 349 with the same OS error 5 in
  `Cannot replace .../preferences.json`. Evidence: `artifacts/preference-before-stress.log`.

The persistence code attempted atomic replacement once and returned immediately
on a transient Windows access failure. There was no retry. The exact external
process denying access in the original run was not identified; the failure does
not depend on attributing it to a particular sync or indexing application.

There was a second behavior issue: a preference-save failure used the manuscript
error field, whose presence inhibits autosave. A later successful preference
save did not clear that field.

## Fix

- Preference reads and writes retry Windows access/sharing errors 5, 32 and 33.
  Backoff is bounded to 235 ms total, with a final attempt; other errors return
  immediately. File I/O itself can take additional time. The retry occurs only
  on preference operations; general manuscript atomic-write behavior is unchanged.
- Each write still uses atomic replacement; it never deletes/truncates the old
  preference file before replacement succeeds. Persistent failure returns an
  error while preserving the previous file and in-memory language.
- Preference errors have their own state and appear in Settings. They do not
  set or clear the manuscript error state or disable manuscript autosave. A
  successful language save clears the preference error.
- Native preference round-trip failures return diagnostic QA errors rather than
  panicking in an unwinding-forbidden native callback.

## Regression evidence

- Before-fix controlled lock test: FAIL with native OS error 5.
- After-fix controlled write lock (released after 80 ms): PASS.
- Persistent write lock: PASS; error returned, old bytes/language preserved, no
  temporary files left after retry exhaustion.
- Temporary exclusive read lock: PASS; English loaded after release rather than
  silently falling back to Vietnamese.
- 2,000 repeated write/read round-trips in a new isolated OneDrive folder: PASS.
  Evidence: `artifacts/preference-after-stress.log` and `artifacts/preference-fixed-path.txt`.
- Workspace tests after version bump: PASS, 69 tests; the explicit stress test
  and Vim oracle remain opt-in tests.
- Clippy with warnings denied: PASS.
- Dependency gate: PASS, 741 registry package versions; no unreviewed advisory;
  existing tracked informational notices unchanged.

The native runner now holds an actual permanent preference-file lock, requests
a language change, and verifies the UI language/file remain English, a settings
error is recorded, and the manuscript error state is untouched. After releasing
the lock it verifies Vietnamese persists, the settings error clears, and the
manuscript content remains unchanged. Final packaged-binary results follow.

## Final packaged-binary acceptance

- Optimized 0.2.7 build and NSIS 3.13 x64 packaging: PASS.
- Packaged EXE SHA-256: `CA0EBE4895044B72D98AEF4432C632DA680AB557E084EEA1E0DF75E489EA2FF9`, identical to the optimized build output; file/product version 0.2.7.
- Installer: 4,768,681 bytes; SHA-256 `3058BF3D1B20FF852F5C262312464CF60FDCF3AA77AB2C4AEDED22BE57EB20D5`. Authenticode remains unsigned as previously requested; update metadata is signed and its signature is verified by the release tool.
- `tools/native-smoke.ps1` against the exact packaged EXE, with isolated manuscripts/preferences inside the original OneDrive workspace: PASS in both load-success and load-failure scenarios. This includes the new permanent-lock/error-state/recovery checks and all existing editor/Vim/IME/save regressions. Saved concurrent edits, queued-close markers and exit 0 were verified for both scenarios.
- The successful run is `artifacts/release-027-native/results.json`; there was no failed native candidate/retry in this 0.2.7 run. Nonfatal GPUI `window not found` close messages are retained in stderr; actual close completion and persisted files were independently checked by the runner.
- Release grid security regressions: PASS, both tests. Final 0.2.7 stress diagnostic: PASS, another 2,000 round-trips in an isolated temporary directory; the earlier fixed-code OneDrive stress run is recorded above.
- Machine-readable acceptance: `artifacts/release-027-acceptance.json`. The installed app and user manuscripts were not changed or closed by these tests.
