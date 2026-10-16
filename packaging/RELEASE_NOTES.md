Oicha 0.2.3 fixes vertical cursor movement with the arrow keys.

After End or a Vim end-of-line motion, Arrow Up/Down now use the cursor's current
horizontal position instead of jumping to the end of a longer row. Moving through
short rows preserves the original horizontal goal and restores it on longer rows.
This works with Vim enabled or disabled. Explicit Vim j/k end-of-line behavior is
preserved; the first-row margin reveal and scroll-boundary fixes from 0.2.2 remain.

The Windows installer remains unsigned with Authenticode; update metadata is signed.
