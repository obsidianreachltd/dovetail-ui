# B0004 — A typed letter appeared twice once, and could not be repeated

| Field | Value |
|---|---|
| ID | B0004 |
| Status | Closed |
| Severity | Medium |
| Milestone | M06 |
| Found on | 2026-11-22 |
| Platform | Windows |

## Summary

Once, typing "Grace Hopper" into a text field in `examples/form` gave "Grace Hoppeer": one extra "e". It could not be repeated, so it is not yet known whether Dovetail or the test tool caused it.

## Steps to reproduce

1. `cargo build -p example-form`, then start `target\debug\form.exe`.
2. Bring its window to the front.
3. Send these keys in one call to Windows' `SendKeys.SendWait`: `{TAB}Grace Hopper{TAB}grace@navy{TAB}First line~Second line{TAB} {TAB}~` (`~` is Enter).
4. Look at the Name field.

## Expected

The Name field shows "Grace Hopper".

## Actual

In the first run, a screen capture showed "Grace Hoppeer". The other fields were right: "grace@navy", the two message lines, the toggle on, and "Check the email address." after Submit.

In eight later runs the text was right every time. In four, "Grace Hopper Mississippi bookkeeper" was typed alone. In the other four, the full sequence above was typed. In each run the field's text was copied out with Ctrl+A and Ctrl+C and compared.

## Notes

- **The extra letter** followed a doubled letter ("pp"). It was not a held key: SendKeys presses and releases each key at once.
- **Possible causes in Dovetail:**
  - With the input method turned on, which it is while a text field has focus, Windows might deliver the same character both as a key event with text and as an `Ime::Commit`. The text field would then insert it twice. Earlier checks with the input method on typed "Copied from Dovetail" correctly, so if this happens, it is rare.
  - A key repeat reaching the text field. UI windows receive repeats, because a held key should repeat in a text field, but nothing here held a key.
- **A possible cause outside Dovetail:** the keys were sent straight after the window was brought to the front, and SendKeys is known to be unreliable then.
- **To find out:**
  - Add a way to log every key and IME event a window receives, with times, such as an environment variable.
  - Then type for a while, both by hand and with a tool, until the fault happens again. Compare the log with the text.
- **Investigated on 2026-11-22. It was not reproduced.**
  - `DOVETAIL_LOG_INPUT` now makes a window write every key, input-method and focus event to stderr, with times, and whether the window used each key. With it set, `examples/form` also logs every change to its fields.
  - A safer test tool sent the same keys 32 more times, in three ways: in one batch with the window on top, a word at a time, and in one batch straight after `AppActivate`, as in the first run. Before every batch it checked that the form's own window was in front.
  - Every run typed "Grace Hopper". Each log has the same 56 key presses: the 44 keys sent, plus the Shift presses that SendKeys adds for capitals and "@". There were no repeats, no synthetic presses and no input-method events.
  - With the 8 earlier runs, that is 40 correct runs against the one failure.
- **Why Dovetail is unlikely to be the cause:**
  - A text field adds text only for a key or input-method event, once for each event.
  - The only time it replaces its text is with a value its owner passes back, and that value can only be one the field sent out earlier. A late update could at worst lose a recent letter, never add one.
  - So an extra letter needs an extra key event. The failing run was the only one without a log, so it cannot show whether one arrived.
  - `keys_that_arrive_between_frames_go_in_once_each` in `dovetail-controls/tests/text_field.rs` now checks that keys reaching a field between frames, with the form rebuilt at different points, each go in once.
- **Closed on 2026-11-22 as cannot be reproduced**, after the 32 logged runs above. Open it again if the fault comes back with a log.
- **If it happens again:** run with `DOVETAIL_LOG_INPUT=1` and keep stderr. Two presses of the letter in the log would point to the input source; one press would point to Dovetail.
- **Testing with sent keys is risky.** In a ninth run the form window did not come to the front. The keys, including Ctrl+A and Ctrl+C, went to another program's window, so that run was not counted. A test that sends keys must check, before every key, that the window in front is the one under test, and stop if it is not.

## Fix

Empty until fixed.
