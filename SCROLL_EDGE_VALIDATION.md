# Scroll edge fixes — 2026-10-16

Candidate: **Oicha 0.2.2, Windows x64**.

- Installer: `dist/Oicha-0.2.2-Windows-x64-Setup.exe`, 4,704,100 bytes.
- Installer SHA-256: `A9F94EC0C0FE178F279F627F3D2476CC93AF02BB3081E6443229F19826DEBDEA`.
- App SHA-256: `C5F7485E5EBAA91427417C85E509669BBBEE9DADBE3DE0E470D2D9D4AB9C9414`.

The first display row now reveals the full top page margin and the space above
the paper during cursor reveal. This applies even when that row's glyphs are
already visible, and also when clicking it. Other display rows keep their existing
visibility rules; `zz` and EOF writing-area advance keep their existing priorities.

Wheel handling previously ran after GPUI's native listener, then subtracted a
delta to replace it with animation. The native listener captured the element's
painted line height; the application callback used the event context's line height.
Those values can differ. The attempted reversal moved the viewport away from a
boundary, followed by an animated return, repeatedly. The old callback-only test
used the same line height on both sides and missed this real dispatch behavior.

A non-occluding viewport sibling now receives wheel events before the built-in
scroll listener, applies one clamped target and stops propagation. It covers the
paper, surrounding padding and tail space, while clicks and selections still pass
to the editor. Precision deltas apply immediately within the same bounds and
cancel pending smooth motion. Compact popup inputs retain GPUI native scrolling.

Verification:

- Reproduced the old behavior with `tools/native-scroll-smoke.ps1 -ExpectFailure`:
  the first row stayed at offset -80, and real `WM_MOUSEWHEEL` messages produced
  nonzero excursions at both scroll boundaries.
- Debug and release probes passed after the fix. The release probe samples 64
  repeated native wheel inputs at top/bottom over text and padding, checks immediate
  reversal and settling, Arrow Up with Vim on/off, first-row mouse clicks, precision
  delta clamping and unchanged manuscript generation. The top offset remains zero;
  all bottom samples remain at the actual maximum. This is Windows event-dispatch
  coverage, not a claim about every mouse driver or hardware touchpad.
- Workspace: **57 tests passed**; the external Vim oracle remains explicitly
  ignored. Formatting and Clippy with warnings denied passed. No dependency
  versions changed beyond the workspace version bump. The advisory scan checked
  741 registry versions with no unreviewed blocking advisory.
- Both release native smoke scenarios passed, covering Vietnamese IME/undo,
  display-row Vim motions, smooth/precision motion, fixed pages, EOF advance,
  modals, concurrent edits/load/save/close and DOCX export. Native caption tests
  passed on the exact installer payload.
- Actual signed upgrade **0.2.1 → 0.2.2** passed, including failed-save blocking,
  unsaved Vietnamese/emoji preservation, English preference, helper restart,
  installed payload hashes and preserved shortcut selections/user files.
  Existing desktop shortcuts were preserved without being overwritten. The full
  shortcut-writing installer suite deliberately refused an existing user shortcut;
  the unchanged NSIS lifecycle was already tested in 0.2.1, and this release used
  the isolated no-shortcut upgrade path instead.

Local evidence, excluded from Git:

- `artifacts/scroll-edges-before/results.json`
- `artifacts/scroll-edges-final-debug/results.json`
- `artifacts/scroll-edges-final-release/results.json`
- `artifacts/scroll-fix-native-release/results.json`
- `artifacts/scroll-fix-caption/results.json`
- `artifacts/windows-update-qa-results.json`
- `artifacts/scroll-edges-dependency-audit.json`

The installer remains unsigned with Authenticode per the existing release policy.
Update metadata is signed with the existing production Ed25519 key.
