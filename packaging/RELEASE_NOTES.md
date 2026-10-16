Oicha 0.2.2 fixes two scrolling issues on Windows:

- Moving the cursor to the first display row now reveals the entire top margin
  and space above the page, including with Arrow Up and mouse clicks.
- Repeated scrolling past the top or bottom no longer makes the page flicker.
  Wheel events are handled once, offsets are clamped before drawing, and reversing
  direction works immediately. Precision touchpad input remains immediate.

Smooth scrolling, Vim motions, the writing space below the last row, Vietnamese/
English interface selection, saved manuscripts and signed updates remain available.
The Windows installer remains unsigned with Authenticode; update metadata is signed.
