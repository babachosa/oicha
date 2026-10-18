Oicha 0.2.6 adds Vim character-find motions.

- Use f{character} to find the next occurrence on the current logical line,
  or F{character} to search backward. Counts work, for example 2fa.
- Press ; to repeat the last character search, or , to repeat in the opposite
  direction without changing the remembered direction.
- Combine these motions with Visual selection and d/c/y operators, including
  counted commands such as d2fa and 2d2fa. Changes remain one undo transaction.
- Vietnamese characters, combining marks and emoji retain safe cursor boundaries.
  Target keys such as /, :, n and 0 remain literal characters during a find.
- A failed or cancelled find never partially moves the cursor or edits text.
  Searches stop at Enter and may cross soft wraps, matching Vim's logical lines.
- The F1 shortcut guide includes these commands in Vietnamese and English.

Validated with 66 workspace tests, 104 differential cases against Vim, Clippy,
and native Windows key dispatch and editor regression checks.

The Windows installer remains unsigned with Authenticode; update metadata is signed.
