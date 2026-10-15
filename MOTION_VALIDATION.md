# Typing and scroll motion — 2026-10-15

The GPUI manuscript renderer now interpolates visual caret movement over 80 ms and programmatic/wheel scroll offsets over 120 ms using a finite cubic ease-out. New single-grapheme insertions fade from 75% to full opacity over 70 ms; existing text never moves or fades. Composition, replacement, paste and whitespace skip the glyph fade. Emoji retain their native color painting.

Document edits, Vim byte offsets, selection, hit testing, undo and the UTF-16/IME candidate bounds update immediately. Only the painted caret and viewport interpolate. New rows start at their own horizontal position instead of sweeping diagonally through paragraphs; jumps across page margins snap the caret's document position while the viewport slides. Mouse selection snaps the caret. Font/wrapping geometry changes reset visual motion.

Rapid retargeting samples the current interpolated position, preserving continuity. An unchanged target does not restart the timer. Autoscroll evaluates visibility against the pending destination to avoid fighting an unfinished Enter/soft-wrap page advance. Wheel events cancel pending automatic reveal; precision touchpads retain GPUI's native deltas/inertia. GPUI 0.2's built-in wheel delta is replaced by smooth motion for discrete wheels, not added twice. External handle changes and clamping cancel old scroll targets. Frame requests stop after motion settles; the existing caret blink timer remains.

Verification:

- 46 workspace tests pass (38 core, 2 security, 2 pagination, 1 glyph indexing, 3 motion). Motion tests cover intermediate position, frame-count independence, retarget/reversal continuity, unchanged targets, finite completion and zero duration.
- Format and Clippy with warnings denied pass. No dependency or lockfile changes.
- Native debug success/failure scenarios pass at `artifacts/motion-debug-2/results.json`, including intermediate/settled caret positions and scroll offsets, precision-scroll cancellation, Vietnamese baseline/IME/undo, A/0/j/k, `zz`, Enter/soft-wrap page advance and fixed A4 page geometry.
- The wheel regression invokes the application callback after reproducing GPUI's built-in delta stage; it does not claim end-to-end coverage of every mouse/driver. Intermediate checks run on actual Windows GPUI frames, not just the standalone easing function.

Release evidence and binary identification are recorded in VALIDATION.md. These local checks establish functional animation and regression coverage; they are not whole input-to-photon latency or frame-time percentile measurements.
