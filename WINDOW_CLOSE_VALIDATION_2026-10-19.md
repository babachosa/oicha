# Caption controls and close choices — 2026-10-19

Local, unpublished change after Oicha 0.2.7, together with the display-row dd fix.

## Behavior

- Removed the titlebar's 4 px top and horizontal padding. Minimize, maximize /
  restore and close keep the native Windows control areas and now fill the
  titlebar height at the top/right client edge.
- X and Alt+F4 request a close decision when there are unsaved changes or no
  writable store. The modal offers Save, Don't Save and Cancel in Vietnamese
  and English; Enter defaults to Cancel and Escape cancels.
- Save explicitly authorizes the existing save-before-close path. Save As only
  opens if required after choosing Save. Cancelling the picker cancels the close
  request. A failed save keeps the app and unsaved content open. `:wq` remains
  an explicit save-and-close command.
- Don't Save keeps the last saved manuscript, discards in-memory edits and a
  compatible unsaved checkpoint, then closes. Foreign, invalid or unreadable
  recovery data is preserved and reported rather than deleted. The last saved
  manuscript is never overwritten by this choice.
- Cancel preserves the editor and clears the pending close. Repeated X does not
  bypass or reset the active choice. The settings launcher cannot replace the
  close prompt; replacing a close modal programmatically cancels its pending
  close intent. Async discard cannot be double-submitted.
- Autosave pauses while a close decision is pending. A document without a store
  no longer triggers a native Save As picker from the autosave timer. Documents
  already fully saved close directly; this does not undo previous autosaves.
- Existing read/save workers finish before the close decision runs. Update
  installation keeps its separately authorized save-before-restart path.

## Verification

- Workspace tests: PASS, 75 tests, including checkpoint discard/reopen and exact
  saved-file preservation, plus foreign/invalid checkpoint preservation.
- Clippy with warnings denied and format: PASS.
- Four isolated native close scenarios: PASS — Save, Don't Save, no store,
  and failed Save due to an external conflict. Each first verifies default-Enter
  cancellation, preservation of edits and no Save As before a user choice.
  Exit 0, actual saved title, close marker and absence of a restorable discarded
  checkpoint are checked outside the app. Initial evidence:
  `artifacts/close-flow-2026-10-19/results.json`.
- Full native editor/save/load regressions: PASS in load-success and load-failure
  scenarios. The queued-close test now explicitly chooses Save after the I/O
  barrier, and still verifies persisted concurrent edits and close completion.
  Initial evidence: `artifacts/caption-close-full-2026-10-19/results.json`.
- Computer Use visual inspection on an isolated QA window: verified the new
  three-choice popup, maximize/restore rendering, and X hover background touching
  the top/right edge. Clicking Save opened the native Save As dialog; Escape
  cancelled it and left the app open. Clicking X again reopened the close modal.
  The owned visual fixture was subsequently terminated for build cleanup; no
  real document or user app instance was closed.

The installed user app and real manuscripts were untouched. Final optimized
binary results are recorded after the final modal-state guard build below.

## Final optimized binary

- Final release build after the settings/close-modal guard: PASS.
- `tools/native-close-smoke.ps1` against `target/release/oicha.exe`: PASS in
  all four cases, with saved-file verification and actual close markers.
  Evidence: `artifacts/caption-close-release-2026-10-19/results.json`.
- `tools/native-smoke.ps1` against the same release EXE: PASS for both
  load-success and load-failure, including the new dd behavior and explicit
  Save choice after queued close during load. Saved concurrent edits and
  close completion were verified; both processes exited 0.
  Evidence: `artifacts/caption-close-release-full-2026-10-19/results.json`.
- Previously observed nonfatal GPUI `window not found` logging during guarded
  close remains in stderr. PASS reports, saved files, exit codes and final
  close markers were verified independently.
- Binary hash and machine-readable acceptance are recorded in
  `artifacts/caption-close-release-2026-10-19/acceptance.json`.

The binary still identifies as 0.2.7 and is for local testing. No new installer,
Git tag, push or release was performed in this turn.
