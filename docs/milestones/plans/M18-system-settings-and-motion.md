# M18 plan — System settings and motion

| Field | Value |
|---|---|
| ID | M18 |
| Status | Planned |
| Overview | [M18 overview](../overviews/M18-system-settings-and-motion.md) |
| Last updated | 2026-11-26 |

## Goal

Read the system settings that shape how an application should behave, such as less motion, text size, caret blink and double-click time, and add a small animation system that respects them.

## Starting point

- **The system's appearance (M14)** is the pattern to follow:
  - `SystemAppearance` in `crates/dovetail-core/src/appearance.rs` holds dark mode, the accent colour and high contrast;
  - `system_appearance()` remembers which components read it, and `set_system_appearance` builds only those again when it changes;
  - the desktop reads it in `platform::appearance()` (Windows, macOS, and GNOME through `gsettings` in `appearance.rs`, whose parsing is tested on every system);
  - it is read again at start, on winit's `ThemeChanged`, and when a window comes to the front (`lifecycle::refresh_appearance`). winit does not pass on Windows' `WM_SETTINGCHANGE`.
- **Settings read today:**
  - the caret blink: `GetCaretBlinkTime` on Windows; a fixed 530 ms on macOS and Linux. `blink_rate()` in `app.rs` reads it once, so a change needs a restart;
  - overlay scroll bars on macOS: `NSScroller`'s `preferredScrollerStyle`, read each time it is needed. It is type-checked but has never run on a Mac (M15);
  - nothing for less motion, the text scale or double-clicks.
- **The session (M20)** takes `Settings { caret_blink, overlay_scrollbars }` when it is made. The double-click time (500 ms) and distance (4 pixels) are constants in `crates/dovetail-session/src/lib.rs`.
- **The clock and timers (M20):** `dovetail_core::now()`, `TestClock` and timers for `Task::after` on the UI thread. `TestUi::advance` moves the clock and fires the timers that fall due.
- **Animation today:**
  - `Image` plays GIF and APNG with a `Task::after` for each frame's delay, and pauses when its `playing` is false (M17). M17 left pausing for less motion to this milestone.
  - Nothing else moves. There is no way to ask for a frame: the desktop draws when something changed, and wakes at the next timer or `wake_after`.
- **What the layout can change:** sizes, padding, margins, colours and alignment in `Style`. There is no opacity and no offset from the laid-out place, so a fade or a slide cannot be drawn yet.
- **The theme (M14):**
  - text styles (`headline`, `title`, `body`, `body_small`, `label`) are fixed sizes in `ThemeData`;
  - controls read them with `theme()`, and the outermost `Theme` gives the body style to the text inside it;
  - a theme change swaps the colours at once. M14 left animating between themes for later.
- **Controls to animate:** `Toggle` moves its knob with `justify` Start or End; `CheckBox` draws its tick or not; `Collapsible` shows its content or not.

## Steps

1. [ ] **Decide the shape**, in a decision record, D0028:
   - how settings are kept and read, with readers for each setting, so a component that reads only less motion is not built again when the caret blink changes;
   - how animations get frames: frame requests in `dovetail-core`, which the desktop answers at the display's rate and `TestUi::advance` answers at 60 frames a second of test time, or timers at a fixed rate;
   - how opacity and offsets are painted, and whether hit testing and the accessibility bounds follow an offset;
   - what the text scale applies to: the theme's text styles only, or also sizes an application sets itself.
2. [ ] **System settings in `dovetail-core`:**
   - `SystemSettings`: less motion, the text scale, the caret blink (or none), the double-click time and distance, and overlay scroll bars;
   - a function for each, such as `reduce_motion()` and `text_scale()`, each with its own readers, as `system_appearance()` has;
   - `set_system_settings`, which builds again only the readers of what changed;
   - defaults where no desktop sets them, as in tests: motion on, scale 1, a 530 ms blink, 500 ms and 4 pixels.
3. [ ] **Read them on each system**, in `crates/dovetail-desktop/src/platform/` and a parser tested everywhere, as `appearance.rs` is:
   - Windows: `SPI_GETCLIENTAREAANIMATION`, the `TextScaleFactor` accessibility setting, `GetCaretBlinkTime`, `GetDoubleClickTime`, and `SM_CXDOUBLECLK` and `SM_CYDOUBLECLK`;
   - macOS: `NSWorkspace`'s `accessibilityDisplayShouldReduceMotion`, `NSEvent`'s `doubleClickInterval`, the insertion point's blink periods, and the scroller style. macOS has no text scale for applications, so it is 1;
   - GNOME: `enable-animations`, `text-scaling-factor`, `cursor-blink` and `cursor-blink-time`, and the mouse's `double-click`. GNOME has no double-click distance, so it is 4 pixels;
   - read them whenever the appearance is read: at start, on `ThemeChanged` and when a window comes to the front.
4. [ ] **The session follows them.** The double-click time and distance move into `Settings`. A window's session gets new settings when they change, so the caret blink changes without a restart, and `blink_rate()`'s cache goes. `TestUi`'s `Options` can set them.
5. [ ] **Frames for animation**, as step 1 decides:
   - a component can ask for the next frame, and is given the frame's time;
   - the desktop draws frames while any are asked for, and none when nothing is;
   - `TestUi::advance` gives frames in test time, and `TestWindows` waits for real ones.
6. [ ] **Animated values:**
   - `use_animated(target, transition)` gives a value that eases from where it is to `target` over the transition's time;
   - a `Transition` has a duration and a curve: linear, ease in, ease out and ease in-out;
   - values that can be eased: numbers, colours, points, sizes and edges, through a trait applications can add to;
   - a new target part of the way through starts from the value shown, not from the old start;
   - with less motion set, a value jumps to its target.
7. [ ] **Opacity and offsets in `Style`**, as step 1 decides, painted by `dovetail-render`, with a GPU test for each.
8. [ ] **The text scale in the theme.** The outermost `Theme` multiplies its text styles by `text_scale()`, so every control that reads them grows. The body text it gives, and text with no style, grows too.
9. [ ] **`Image` and less motion.** With less motion set, an animated `Image` shows its first frame and does not play, unless the application asks it to play anyway.
10. [ ] **Controls that move**, each with a headless test that moves the clock through it, and one with less motion set:
    - `Toggle`: the knob slides and the track's colour fades;
    - `CheckBox`: the tick fades in;
    - `Collapsible`: the content opens and closes;
    - `Theme`: the colours fade from one theme to the next.
11. [ ] **An example**, `examples/motion`, which shows each setting's value and the moving controls. Its `--self-test`, with `TestWindows`:
    - sets each setting from inside the program and checks the result;
    - checks that an animation ends at its time, within a frame;
    - checks, with `WindowStats`, that no frames are drawn while nothing moves.
12. [ ] **Documents:** D0028, the testing guide (frames in `advance`), the crates page, the glossary (less motion, text scale, transition), and AGENTS.md for animating with `use_animated` and respecting less motion.

## Prototypes and open decisions

- **The shape of settings, frames, opacity and offsets, and the text scale:** decided in D0028 (step 1), before steps 2 to 8 are committed.
- **Seeing a change while the application is in front.** Settings are read when a window comes to the front, as the appearance is. That covers changing them in the system's settings window. A change made some other way while the application stays in front, such as by `gsettings` in a terminal, is seen only at the next focus. Decide in step 3 whether that is enough, or whether each system's change notification is needed. On Windows that is `WM_SETTINGCHANGE`, which winit does not pass on.

## Risks

- **A fading theme builds every reader of the theme again on each frame**, which is most of the window. Measure the frame time while a theme fades in the themes example. If it is too slow, fade only the window's colours and change the rest at the end.
- **Opacity of a group of elements** is not the same as making each element see-through: where children overlap, they show through each other. Step 1 decides whether that is acceptable for fades, or whether a group is drawn to a texture first.
- **Windows' text size setting** is meant for apps that ask for it, and many Win32 programs ignore it. Check by hand that reading `TextScaleFactor` gives the value the Settings app shows, and that it changes while the application runs.
- **Reading GNOME's settings runs `gsettings` once for each setting**, now about ten times when a window comes to the front. If it is slow, read them with one `gsettings list-recursively` for each schema.
- **macOS code that has never run.** The scroller style has only been type-checked, and the new settings will be the same. The manual checks below need a Mac.
- **Animations can make tests slow or flaky.** Headless tests move the clock and never wait. The example's self-test uses short durations, and a time limit on each step.

## Testing

- **Settings:**
  - the parsers for each system's values, on every system, as for the appearance;
  - a test that reads each setting in a component and checks that only that component is built again when it changes, and others are not (criterion 2).
- **Animation, headless with `TestUi`:**
  - an animated value ends at its time, within one frame, for each curve (criterion 3);
  - a new target part of the way through starts from the value shown;
  - with less motion set, values jump and `Image` does not play (criterion 4);
  - each control's transition, frame by frame.
- **Painting:** a GPU test for opacity and offsets.
- **Real windows:** the example's self-test in CI on all three systems: each setting, an animation's end, and no frames while nothing moves (criteria 3 and 5).
- **By hand, on Windows, Linux (GNOME) and macOS**, with the example running (criterion 1). Change each in the system's settings:
  - less motion: the controls stop sliding and the animated image stops;
  - text size (Windows and GNOME): the text grows;
  - caret blink rate, and blinking off: the caret follows;
  - double-click time: a slow double-click selects a word only when the time allows it;
  - on macOS, "Show scroll bars": the bars follow.

## Done when

Copied from the overview.

1. Changing less motion, the text scale, the caret blink and the double-click time on the system changes a running application, on Windows, Linux and macOS.
2. A component that reads a setting is built again when it changes, and components that do not read it are not, checked by a test.
3. An animated value reaches its end at the time asked, within one frame, and no frames are drawn when nothing is animating.
4. With less motion set, transitions jump to their end and animated images pause.
5. The example's self-test checks each setting and the animations.
