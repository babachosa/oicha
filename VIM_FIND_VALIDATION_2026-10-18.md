# Vim character find — 2026-10-18

Validation for the Oicha 0.2.6 release candidate after v0.2.5.

## Behavior

- `f{character}` searches forward and `F{character}` backward, excluding the current character. Searches stop at a logical newline and can cross soft wraps.
- Counts, operator counts, Normal, Visual and Visual Line are supported. Forward operator motions include the target; backward motions include the target and exclude the original cursor character, matching Vim.
- `;` repeats the last search; `,` reverses it without changing the remembered direction. Counts and operators work with both.
- A failed or insufficient counted search leaves the cursor, text, selection, register and history unchanged, and clears the pending operator. Escape cancels the pending target.
- Targets are consumed before command/count parsing: `/`, `:`, `0`, `n`, `f`, punctuation, spaces and tabs remain literal targets. Unicode movement and operator ranges remain on grapheme boundaries, including Vietnamese NFC/NFD and emoji.
- F1 help includes the commands in Vietnamese and English.

## Verification

- `cargo fmt --all --check`: PASS.
- `cargo test --workspace --locked`: PASS, 66 tests.
- `cargo test -p oicha-core --test vim_reference --locked -- --ignored`: PASS, all 104 cases against the installed Vim, including 35 new character-find cases.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS.
- `cargo build -p oicha --locked`: PASS.
- `cargo build -p oicha --release --locked`: PASS for the initial implementation. Release packaging checks for 0.2.6 are recorded below.
- `tools/native-smoke.ps1 -Executable target/debug/oicha.exe -OutputDirectory artifacts/vim-find-2026-10-18`: PASS in both load-success and load-failure scenarios, including saved concurrent edits and queued close markers, exit 0.

The native runner now exercises `f/`, `f:`, `fn`, `f0`, a Vietnamese target, counted backward find, repeat/reverse, failed count, `cf` and undo through GPUI's actual key dispatch. The installed application and the user's manuscripts were not changed. This tests GPUI dispatch, not a new physical UniKey/EVKey keyboard-hook validation.

## Final 0.2.6 packaging

- Re-ran format, all 66 workspace tests, all 104 Vim reference cases, Clippy and the optimized build after the version bump: PASS.
- Release security regressions: PASS, both grid tests.
- Dependency gate: PASS, 741 registry package versions checked, no unreviewed advisory. Ten explicitly tracked INFO Unmaintained IDs produce 13 package-version findings. The three newly issued zbus-family notices were reviewed against RustSec and documented in DEPENDENCY_POLICY.md; none is in the Windows dependency tree. Registry dependency versions were not changed.
- NSIS 3.13 x64 installer: 4,766,816 bytes, unsigned Authenticode as previously requested.
- Installer SHA-256: `E5C970C0D7C84F5D46698295ACB86C72313B0DDF6CA6B35B48AF0394656903C8`.
- Packaged EXE SHA-256: `4F1113DB16A7EA026E329322003342C2798B25441F2CEA373AB1C23A041E42D0`, identical to the optimized build output; file/product version 0.2.6.
- Update metadata signed and signature verified against the application's public key by the release tool.
- Native regression run against the exact packaged EXE: PASS for load-success and load-failure, exit 0, saved concurrent edits and queued-close markers verified. Evidence directory is recorded in `artifacts/release-026-recheck-path.txt`; receipt in `artifacts/release-026-acceptance.json`.

One initial native run under the OneDrive workspace passed load-success but
aborted in the load-failure fixture's preference round-trip assertion (En
remained persisted when Vi was expected). It did not reach the new find tests.
That failed run is preserved in `artifacts/release-026-native/` and is not counted
as a pass. The same, unchanged packaged EXE passed both scenarios using fresh
isolated QA files under LocalAppData. The cause of the failed round-trip
was not captured or proven. In normal application flow, `set_language` surfaces
a preference-save error before changing the in-memory language; the panic here
was the smoke fixture's assertion. The successful recheck also emitted the
previously observed nonfatal GPUI `window not found` message during guarded close;
exit codes, saved files, PASS reports and close markers were checked separately.
