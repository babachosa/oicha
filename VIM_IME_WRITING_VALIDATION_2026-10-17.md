# Vietnamese input and writing viewport — 2026-10-17

Candidate: Oicha 0.2.5, Windows x64, Rust 1.98.1, GPUI 0.2.2.

## Root causes and changes

UniKey maintains its own spelling buffer before GPUI receives keyboard events.
Consuming a Vim command in `TextEditor::on_key` does not remove that command from
UniKey's buffer. An `i` can therefore contaminate subsequent Telex input, while
`w` can be transformed/swallowed before the editor ever sees a command. This was
reproduced with the installed UniKey 4.6 RC2 in Unicode/Telex mode: `i`, followed
by physical `h o a f` inside existing text, inserted raw `hoaf`. A native function
key context boundary allowed the same input to produce `hòa`.

The Windows bridge now routes printable Normal/Visual commands directly to the
foreground Oicha editor, ahead of external keyboard hooks. It does not route
Insert-mode input, Vim-off input, popup input, modified shortcuts, or another
foreground application's input. It retains the matching key-up and repeat state.
Dispatch is synchronous on the UI thread so entering Insert takes effect before
the next key. Physical Shift is reflected temporarily during dispatch because a
low-level hook runs before Windows updates the queue's keyboard state; without
this, fast `G`/`A` could be decoded as lowercase. The original state is restored.

Entering Insert also sends a non-text context boundary, guarded by an exact
foreground HWND check. No manuscript space, Backspace, caret move, or change to
the user's Vietnamese/English setting is involved. The hook is removed on exit.
The application remains Rust-only, with the existing GPUI renderer.

Relevant upstream interfaces: [UniKey context reset contract](https://github.com/fcitx/fcitx-unikey/blob/master/unikey/unikey.h),
[Windows low-level keyboard callback](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelkeyboardproc).
No UniKey engine code is incorporated into Oicha.

Writing-area follow previously required `cursor == text.len()`. Enter and soft
wraps inside existing prose consequently followed a different rule. Both now
share the same viewport threshold (65%) and destination (32%), regardless of
whether text follows the caret. Vim `o`/`O` use that rule as well. Ordinary edits
within the same row retain the viewport.

There was a second timing issue: GPUI may draw a dirty frame between two key
dispatches. A newline's scroll animation could already have started by the time
the next navigation key arrived. Clearing only a pending boolean did not stop
that old animation. Navigation, mouse placement and history now cancel the old
follow request and motion before revealing the new caret. Wheel input also clears
an unconsumed edit-follow request. Immediate text/IME input preserves the request.

The Save toolbar label is now constant across saving-state changes. Progress
remains in the status bar, preventing the Save/Save as buttons from moving during
autosave. The sheet gap changes from 24 to 12 px; A4 geometry and margins remain.

## Checks completed

- Workspace tests: 62 passed. Formatting and Clippy with warnings denied passed.
- Real Vim differential test passed, including Vietnamese word joining and
  `dw`/insert/word-motion cases added to the reference suite.
- Strict dependency gate passed for 741 registry package versions. No new
  advisory exception or registry package version was introduced. The existing
  reviewed grid backport hash is unchanged.
- Native debug smoke passed with middle-document Enter, burst soft wraps,
  navigation arriving before the next paint, immediate IME after Enter,
  fixed-page geometry, Vim ON/OFF, history, modal lifecycle, save and DOCX export.
  Evidence: `artifacts/vim-ime-investigation/cancel-fixed.*`.
- Actual key input through the installed UniKey passed 36 cases: 12 basic cases,
  20 repeated Shift+G/A cases, and four additional cases covering `niềm`, `đắng`,
  Visual deletion and 50 consecutive `w` commands. Expected text and byte cursor
  offsets are stored with each fixture in
  `artifacts/vim-ime-investigation/shift-fixed-real-key-results.json`.
  Search-popup input was also visually verified to produce `niềm` via Telex.

## Investigation records

`before.*` records the published 0.2.4 reproduction. `after.*`, `bridge.*` and
`final-debug.*` are intermediate experiments, not final release evidence. A
context-boundary-only approach was insufficient for `w`; a later experiment
exposed the Shift timing issue. Incorrect byte offsets in an early fixture were
corrected before recording the successful matrix. `cancel-debug.*` and
`cancel-diagnostic.*` exposed the already-started motion described above; the
unchanged viewport assertion passed after cancellation was fixed.

## Release validation

- Final optimized release build passed, followed by both native load-success and
  load-failure scenarios. Both exited 0, preserved edits made during I/O and
  completed the queued close. Their DOCX files each passed independent .NET ZIP
  and XML parsing of five parts. Evidence: `release-native-results.json` and
  `release-load-success.*` / `release-load-failure.*` in the investigation folder.
- Native assertions include the Enter/navigation timing regression and counted
  `2zz` centering after window resizing, in addition to existing editor coverage.
- The packaged release EXE passed the same 36 actual UniKey key-input cases.
  Evidence: `artifacts/vim-ime-investigation/release-keys-real-key-results.json`.
- Both release security regression tests passed. Final workspace tests, Clippy
  and formatting passed; logs are in `final-tests.log`, `final-clippy.log` and
  `final-release-security.log` in the investigation folder.
- NSIS installer build passed. Version/icon resources and bundled Microsoft CRT
  signatures were checked. Its EXE payload hash equals the native-tested EXE.

| Artifact | Value |
| --- | --- |
| Installer | `dist/Oicha-0.2.5-Windows-x64-Setup.exe` |
| Bytes | 4,759,187 |
| Installer SHA256 | `9A7BC1DDA4F5BF0CA47A5AB7FC784D1F7B7E43A9010FC183A74742854DBE6321` |
| EXE SHA256 | `839B63A4D35711CA9B2DC633C6CACC333E78B99F818AFDBE175171D2325502C4` |
| Authenticode | Unsigned, as previously requested |

`artifacts/vim-ime-investigation/release-acceptance.json` records package and
native results. The two full native runs logged a nonfatal GPUI `window not found`
message after their guarded close; saved-file assertions, close markers and exit
codes passed. No new installer upgrade/uninstall cycle was run against the user's
live installation. The installed app and manuscripts were preserved; all own QA
windows were closed. These hashes describe the locally tested artifacts. Public
release assets are built and update metadata is signed by the repository's
Windows release workflow when the matching version tag is pushed. EVKey and
other input methods have not been exercised in this session.
