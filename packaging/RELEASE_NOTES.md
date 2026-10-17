Oicha 0.2.5 fixes Vietnamese input and Vim command interference on Windows.

Normal/Visual commands now reach Vim before an external keyboard-hook input
method can change or swallow them. Entering Insert starts a fresh input context,
so Vietnamese Telex works after i/a/A/cw/dw and when inserting directly beside
existing text. Repeated w commands jump on the first press after joining words.
Input in Insert mode, Vim off, prompts and other applications uses the OS path.

Enter and soft wraps now open comfortable writing space near the bottom of the
viewport both at EOF and while inserting into an existing paragraph. Rapid input
and IME commits preserve the pending scroll request. Fixed page sizes are retained.

The Save button keeps a stable label during autosave, preventing the toolbar
from shifting. Saving progress remains in the status bar. The gap between sheets
is reduced from 24 to 12 pixels.

The Windows installer remains unsigned with Authenticode; update metadata is signed.
