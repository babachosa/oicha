Oicha 0.2.4 resolves all three findings from the October 17 audit.

Undo, redo and clipboard edits now leave Vim Visual/Visual Line selection state
valid, including Unicode text and undoing to an empty chapter. This fixes a crash
when an old selection anchor pointed past the restored text. Visual paste replaces
the complete visible selection.
If Windows temporarily locks the clipboard, Cut now keeps the text and shows a
retry notice instead of deleting text whose clipboard copy could not be verified.

Home/End and Shift+Home/End follow the current displayed row, with Vim enabled or
disabled. End stays on the same row at a soft wrap, including repeated presses.
Existing Vim logical motions keep their documented behavior.

The ZIP dependency used for DOCX export is upgraded from 4.6.1 to 9.0.4, removing
the three ZIP extraction advisories reported by the audit. The dependency gate
remains strict; no new advisory exception was added.

The Windows installer remains unsigned with Authenticode; update metadata is signed.
