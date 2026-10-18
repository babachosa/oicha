Oicha 0.2.7 fixes intermittent language preference persistence on Windows.

- Brief Windows access/sharing errors during atomic replacement of the settings
  file are retried with bounded backoff. The previous file remains intact.
- Preference reads also retry brief access/sharing errors rather than immediately
  falling back to the default language.
- A persistent settings-save failure keeps the previous language and displays an
  error in Settings. It does not block manuscript autosave, and a successful retry
  clears the settings error.
- Native QA reports preference failures with diagnostic details instead of
  aborting in a native callback assertion.

The failure was reproduced with OS error 5 during repeated writes in OneDrive
and with a controlled Windows file lock. Regression checks cover lock release,
persistent lock preservation, temporary read locks and 2,000 preference changes.
The Vim f/F, ; and , commands introduced in 0.2.6 are retained.

The Windows installer remains unsigned with Authenticode; update metadata is signed.
