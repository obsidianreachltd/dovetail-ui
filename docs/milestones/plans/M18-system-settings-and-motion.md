# M18 plan — System settings and motion

| Field | Value |
|---|---|
| ID | M18 |
| Status | In progress |
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

1. [x] **Decide the shape**, in a decision record, [D0028](../../decisions/D0028-system-settings-and-animation.md), accepted on 2026-11-26:
   - how settings are kept and read, with readers for each setting, so a component that reads only less motion is not built again when the caret blink changes;
   - how animations get frames: frame requests in `dovetail-core`, which the desktop answers at the display's rate and `TestUi::advance` answers at 60 frames a second of test time, or timers at a fixed rate;
   - how opacity and offsets are painted, and whether hit testing and the accessibility bounds follow an offset;
   - what the text scale applies to: the theme's text styles only, or also sizes an application sets itself.
2. [x] **System settings in `dovetail-core`:**
   - `SystemSettings`: less motion, the text scale, the caret blink (or none), the double-click time and distance, and overlay scroll bars;
   - a function for each, such as `reduce_motion()` and `text_scale()`, each with its own readers, as `system_appearance()` has;
   - `set_system_settings`, which builds again only the readers of what changed;
   - defaults where no desktop sets them, as in tests: motion on, scale 1, a 530 ms blink, 500 ms and 4 pixels.
   - Done on 2026-11-26, in `crates/dovetail-core/src/settings.rs`:
     - `SystemSettings`, with `reduce_motion`, `text_scale`, `caret_blink`, `double_click_time`, `double_click_distance` and `overlay_scrollbars`, and the defaults above;
     - `reduce_motion()`, `text_scale()`, `caret_blink()`, `double_click_time()`, `double_click_distance()` and `overlay_scrollbars()`, each with its own readers, and `system_settings()`, which reads them all;
     - `set_system_settings` builds again only the readers of the settings that changed. It keeps the text scale between `MIN_TEXT_SCALE` (0.5) and `MAX_TEXT_SCALE` (3), takes a scale that is not a number as 1, and a negative double-click distance as 0.
     - Two unit tests check the defaults and the odd values. `crates/dovetail-core/tests/settings.rs` mounts four components that read less motion, the text scale, everything and nothing. Changing one setting builds again only its reader and the one that reads everything, and setting the same values again builds nothing (criterion 2).
     - Nothing reads the settings yet: the desktop, the session and the theme come in steps 3, 4 and 8.
     - The fmt and clippy checks, and the workspace tests, pass on Windows.
3. [x] **Read them on each system**, in `crates/dovetail-desktop/src/platform/` and a parser tested everywhere, as `appearance.rs` is:
   - Windows: `SPI_GETCLIENTAREAANIMATION`, the `TextScaleFactor` accessibility setting, `GetCaretBlinkTime`, `GetDoubleClickTime`, and `SM_CXDOUBLECLK` and `SM_CYDOUBLECLK`;
   - macOS: `NSWorkspace`'s `accessibilityDisplayShouldReduceMotion`, `NSEvent`'s `doubleClickInterval`, the insertion point's blink periods, and the scroller style. macOS has no text scale for applications, so it is 1;
   - GNOME: `enable-animations`, `text-scaling-factor`, `cursor-blink` and `cursor-blink-time`, and the mouse's `double-click`. GNOME has no double-click distance, so it is 4 pixels;
   - read them whenever the appearance is read: at start, on `ThemeChanged` and when a window comes to the front.
   - Done on 2026-11-26:
     - `crates/dovetail-desktop/src/system_settings.rs` turns each system's values into `SystemSettings`: `from_windows`, `from_macos` and `parse_gnome`. It is built on every system, and three tests check each system's rules, including values that could not be read, which keep the defaults.
     - Windows (`platform/windows.rs`): `SPI_GETCLIENTAREAANIMATION`, `TextScaleFactor`, `GetCaretBlinkTime`, `GetDoubleClickTime`, and `SM_CXDOUBLECLK` and `SM_CYDOUBLECLK`. Windows' double-click rectangle is centred on the first click, so the distance is half its size: 2 pixels with the usual 4, where the session's constant is 4 today.
     - macOS (`platform/macos.rs`): `accessibilityDisplayShouldReduceMotion`, `doubleClickInterval`, the `NSTextInsertionPointBlinkPeriodOn` default and the scroller style. macOS has no setting to stop the caret blinking that applications can read, so it always blinks.
     - GNOME: `enable-animations`, `text-scaling-factor`, `cursor-blink`, `cursor-blink-time` (a whole blink, so the caret stays on for half of it) and the mouse's `double-click`, with `gsettings`. That is five more runs of `gsettings` each time a window comes to the front.
     - `lifecycle::refresh_appearance` sets the settings with the appearance: at start, on `ThemeChanged` and when a window comes to the front.
     - On this Windows machine they read: motion on, scale 1, a 530 ms blink, 500 ms and 2 pixels. Nothing uses them yet: the session follows them in step 4.
     - The fmt check, clippy for Windows, macOS and Linux, the workspace tests and all 12 examples' self-tests pass on Windows. The macOS and GNOME code has only been type-checked; the manual checks are in Testing.
4. [x] **The session follows them.** The double-click time and distance move into `Settings`. A window's session gets new settings when they change, so the caret blink changes without a restart, and `blink_rate()`'s cache goes. `TestUi`'s `Options` can set them.
   - Done on 2026-11-26:
     - `dovetail_session::Settings` has the caret blink, `double_click_time`, `double_click_distance` and `overlay_scrollbars`, now a value instead of a function to call. It is made `From` a `SystemSettings`, and its default is theirs. The session's double-click constants are gone.
     - `Session::set_settings` gives a session new settings. A new blink rate starts at once, the caret stays on when blinking stops, and it says when the window must be drawn again.
     - The desktop makes each session from `system_settings()`. `Shell::refresh_system`, at each `ThemeChanged` and each time a window comes to the front, reads the system again and gives every window's session the new settings, and draws again the windows whose caret or scroll bars change. `blink_rate()`, `platform::caret_blink_time` and `platform::overlay_scrollbars` are gone; the scroll bar style is now read with the other settings, not each time it is needed.
     - `TestUi`'s `Options::settings` is now a `SystemSettings`, installed for the test's thread, and put back to the defaults when the `TestUi` is dropped. A test changes them with `set_system_settings`, and the window follows at its next update, as a real one does.
     - A gap from M20 was found and fixed: `TestUi::advance` fired timers but not the session's own deadlines, so the caret did not blink in tests, though the testing guide says it does. It now ticks the session at each deadline on the way, in order.
     - Two tests in `crates/dovetail-testing/tests/settings.rs`: clicks 400 ms apart are a double-click with the usual 500 ms and two single clicks with 300 ms; clicks 3 pixels apart are two single clicks with a distance of 2. The caret blinks at 530 ms, stays on when blinking is turned off, and changes to a 1000 ms blink at once.
     - On Windows, double-clicks now use Windows' distance: 2 pixels each way with the usual setting, where it was 4.
     - The fmt check, clippy (with every feature, and for macOS and Linux), the workspace tests, the picture and window tests, and all 12 examples' self-tests pass on Windows.
5. [x] **Frames for animation**, as step 1 decides:
   - a component can ask for the next frame, and is given the frame's time;
   - the desktop draws frames while any are asked for, and none when nothing is;
   - `TestUi::advance` gives frames in test time, and `TestWindows` waits for real ones.
   - Done on 2026-11-26:
     - `crates/dovetail-core/src/frame.rs`: `request_frame()`, called while a component builds, asks for it to be built again at the next frame; outside a build it does nothing. `frame_requested()` says whether any component is waiting, and `start_frame()` marks them to be built again, so each must ask again for the frame after.
     - The desktop, before it waits, starts a frame while one is asked for, no sooner than three quarters of the display's frame after the last (`Shell::frame_gap`, from the monitor's refresh rate, or 60 a second). Showing the frame waits for the display, which keeps the pace. When nothing asks, nothing is drawn.
     - `TestUi::advance` gives a frame every `FRAME`, a 60th of a second of test time, counted from the last frame or from the build that first asked, in order with timers and the session's own deadlines.
     - `crates/dovetail-testing/tests/frames.rs`: a component that moves for 100 ms is built at 17, 33, 50, 67, 83 and 100 ms, each within 10 µs of its frame, and ends within one frame of the time asked. After that nothing is built, through another second. Moving the clock 10 ms gives no frame; 10 ms more gives the one at 17 ms.
     - A real-window test in `tests/windows.rs`: a component that moves for 300 ms drew 16 or 17 frames in three runs here, where a 60 Hz display shows 18, and none in the 300 ms after it stopped.
     - The fmt check, clippy (with every feature, and for macOS and Linux), the workspace tests, the window tests and all 12 examples' self-tests pass on Windows.
6. [x] **Animated values:**
   - `use_animated(target, transition)` gives a value that eases from where it is to `target` over the transition's time;
   - a `Transition` has a duration and a curve: linear, ease in, ease out and ease in-out;
   - values that can be eased: numbers, colours, points, sizes and edges, through a trait applications can add to;
   - a new target part of the way through starts from the value shown, not from the old start;
   - with less motion set, a value jumps to its target.
   - Done on 2026-11-26, in `crates/dovetail-core/src/animate.rs`:
     - `Curve` (`Linear`, `EaseIn`, `EaseOut`, `EaseInOut`, the cubic curves CSS uses), `Transition` (a duration and a curve, `Transition::NONE`, `Transition::millis`) and `use_animated`. The facade's prelude has them.
     - `Animate` for `f32`, `f64`, `Color` (in linear light, with alpha multiplied in, so a colour fading in from clear does not pass through black), `Edges`, pairs, triples and arrays. Points and sizes are pairs.
     - `use_animated` keeps where it is going in a cell the component does not read as a signal, works out its value from `now()`, and asks for a frame until it arrives. It reads `reduce_motion()`, so it jumps when less motion is set, even part of the way through.
     - A gap from step 5 was found and fixed: a component built again for another reason before its frame kept its old request, so one more frame was drawn than needed. A build now drops the component's request, and so does removing it.
     - Five unit tests check the curves against CSS's values, colours in linear light, and each part of pairs, arrays and edges. `crates/dovetail-testing/tests/animate.rs` checks that each curve ends at its time, within one frame, and only moves forward; that a linear value is half way at half the time; that a new target part of the way through turns back from the value shown, with the full time again; that values jump with less motion set; and that a colour is half way in light at half the time (criteria 3 and 4).
7. [x] **Opacity and offsets in `Style`**, as step 1 decides, painted by `dovetail-render`, with a GPU test for each.
   - Done on 2026-11-26:
     - `Style::offset` and `Style::opacity`, which `Style::affects_layout` counts, so layout runs again and reuses the sizes it has. Layout moves a box by its offset when it places it, with everything inside, and multiplies opacity down the tree into `LayoutBox::opacity`. A layer takes the opacity of the element it belongs to. The parent's clip still applies to a moved box. A change of opacity is damage, so it is painted again.
     - The painter multiplies the alpha of backgrounds, borders, the selection, the caret, text, icons and images by the box's opacity. An icon drawn as an image keeps its colour and fades with the image's own opacity, so it is not drawn again for each opacity. Canvases are not faded.
     - Two GPU tests in `crates/dovetail-render/tests/paint.rs`: an offset moves a box and its child, leaves nothing behind, repaints only part of the window and is still clipped by its parent; opacity fades a box and its child, multiplies down the tree, and draws nothing at 0. A headless test in `tests/animate.rs` checks that clicks and the accessibility bounds follow an offset.
8. [x] **The text scale in the theme.** The outermost `Theme` multiplies its text styles by `text_scale()`, so every control that reads them grows. The body text it gives, and text with no style, grows too.
   - Done on 2026-11-26:
     - `TextTheme::scaled`. The outermost `Theme` scales its text styles by `text_scale()`, once, as it applies high contrast once. With no `Theme` at all, `theme()` gives the light theme with its text scaled too.
     - `crates/dovetail-testing/tests/text_scale.rs`: at a scale of 1.5, plain text and a button's label grow by half, the button grows to fit, and a size the application set (`Text { size: Some(24.0) }`) does not change. Back at 1, the text is as before.
9. [x] **`Image` and less motion.** With less motion set, an animated `Image` shows its first frame and does not play, unless the application asks it to play anyway.
   - Done on 2026-11-26: `Image` has `always_play`. With less motion set and no `always_play`, an animation goes back to its first frame and stops its timer. When motion comes back, it plays from the start. Two tests in `crates/dovetail-controls/tests/media.rs` check both (criterion 4).
10. [x] **Controls that move**, each with a headless test that moves the clock through it, and one with less motion set:
    - `Toggle`: the knob slides and the track's colour fades;
    - `CheckBox`: the tick fades in;
    - `Collapsible`: the content opens and closes;
    - `Theme`: the colours fade from one theme to the next.
    - Done on 2026-11-26:
      - `ThemeData::motion`, a `Motion` with short (120 ms) and medium (250 ms) times and the ease-out curve, and `Motion::none()`.
      - `Toggle` slides its knob with an offset and mixes its track's colours, over the short time. `CheckBox` fades its fill and tick in and out, and its border out as the fill comes in. Each animates how far the change has gone, from 0 to 1, and mixes the theme's colours with it. Animating the colours themselves held a switch's colour back for a transition of its own after each step of a theme fade: a fade across 200 controls took 23 frames instead of 15.
      - `Collapsible` fades its content in and slides it down 8 pixels as it opens, and back as it closes, over the medium time. The content is there at once when it opens, for keys and assistive tools, and stays until it has faded when it closes. It is laid out at its full height at once: easing the height needs the content's size before layout, which nothing gives yet.
      - The outermost `Theme` fades its `ColorScheme`, which has `Animate`, over the medium time. It keeps the theme it last gave apart from its signal, so each step of a fade builds the readers again but not the `Theme` itself.
      - `crates/dovetail-controls/tests/motion.rs` steps each control through its transition frame by frame. The knob only moves forward, and arrives within a frame of the short time. The tick fades in and out, then goes. The section fades in and out. The window's background fades from light to dark, and ends on time. With less motion set, all of them jump and nothing asks for frames.
      - Tests that check what a change shows, not how it moves, set less motion: the theme tests, which use a `Tree` with no frames, and the property grid's section test. The toggle tests in `controls.rs` move the clock past the slide.
      - **The fading theme's cost (Risks):** a page of 200 controls, in 40 rows of a button, a switch, a check box, a text field and text, took 2.1 ms a frame on average and 2.4 ms at most to build and lay out while the theme faded. That was in a release build, with no painting. It is well inside a 60 Hz frame, so the whole theme fades.
11. [x] **An example**, `examples/motion`, which shows each setting's value and the moving controls. Its `--self-test`, with `TestWindows`:
    - sets each setting from inside the program and checks the result;
    - checks that an animation ends at its time, within a frame;
    - checks, with `WindowStats`, that no frames are drawn while nothing moves.
    - Done on 2026-11-26:
      - The example shows each setting's value and a switch, a check box and a section that share one value. It also has a box that "Move" slides 200 pixels in 300 ms, a text field, a pad that shows the click count of each press, and an animated picture.
      - The self-test sets each setting from inside the program:
        - with less motion, the box jumps in one build, and no frames are drawn in 400 ms;
        - a text scale of 1.5 grows the text from 18.6 to 27.9 pixels high;
        - the caret blink off and at 1000 ms, the double-click distance and overlay scroll bars reach the values shown;
        - two clicks 150 ms apart are a double-click with 500 ms, and two single clicks with 50 ms, in the real window.
      - The box's move took 304 to 314 ms in three runs here, in 15 to 17 frames, never more than one frame late. The check allows the longest gap between two of its frames, so a slower display passes as long as the move ends within one of its own frames.
      - With motion on, the picture draws frames of its own. So "no frames while nothing moves" is checked with less motion set, when the picture is still too. The real-window test from step 5 checks it with motion on.
      - To let the self-test set the settings, the desktop now gives each window's session the settings before it waits, not only when a window comes to the front (`Shell::pass_settings`). It is one comparison when nothing changed.
      - CI runs the self-test on all three systems, after the window tests.
12. [x] **Documents:** D0028, the testing guide (frames in `advance`), the crates page, the glossary (less motion, text scale, transition), and AGENTS.md for animating with `use_animated` and respecting less motion.
    - Done on 2026-11-26:
      - The testing guide says how animations move with `advance`, and what a test with its own `Tree` must do.
      - The crates page says what each crate gained.
      - The glossary has animated value, frame, less motion, text scale and transition.
      - AGENTS.md says to animate with `use_animated`, to take transitions from `theme().motion`, and to animate how far a change has gone.
      - The README lists the example.
      - D0028 is Accepted and was not edited: the work followed it.

## Prototypes and open decisions

- **The shape of settings, frames, opacity and offsets, and the text scale:** decided in [D0028](../../decisions/D0028-system-settings-and-animation.md) (step 1), accepted on 2026-11-26:
  - `SystemSettings` in core, with readers for each setting;
  - frames asked for with `request_frame()`, drawn at the display's rate, and stepped at 60 a second by `TestUi::advance`;
  - `use_animated` with a `Transition` and the `Animate` trait, jumping when less motion is set;
  - `Style::offset` and `Style::opacity`, worked out by layout, so hit testing and accessibility follow;
  - the text scale applied to the theme's text styles, not to sizes an application sets.
- **Seeing a change while the application is in front.** D0028 reads settings when the appearance is read, including when a window comes to the front. That covers changing them in the system's settings window. A change made some other way while the application stays in front, such as by `gsettings` in a terminal, is seen at the next focus. Each system's change notification (`WM_SETTINGCHANGE` on Windows, which winit does not pass on) waits until a need is found.

## Risks

- **A fading theme builds every reader of the theme again on each frame**, which is most of the window. Measure the frame time while a theme fades in the themes example. If it is too slow, fade only the window's colours and change the rest at the end.
- **Opacity of a group of elements** is not the same as making each element see-through: where children overlap, they show through each other. Step 1 decides whether that is acceptable for fades, or whether a group is drawn to a texture first.
- **Windows' text size setting** is meant for apps that ask for it, and many Win32 programs ignore it. Check by hand that reading `TextScaleFactor` gives the value the Settings app shows, and that it changes while the application runs.
- **Reading GNOME's settings runs `gsettings` once for each setting**, now about ten times when a window comes to the front. If it is slow, read them with one `gsettings list-recursively` for each schema.
- **macOS code that has never run.** The scroller style has only been type-checked, and the new settings will be the same. The manual checks below need a Mac.
- **Animations can make tests slow or flaky.** Headless tests move the clock and never wait. The example's self-test uses short durations, and a time limit on each step.
- **Settings a program sets can be overwritten.** The desktop reads the system's settings again when a window comes to the front. So a self-test that sets its own sees the system's again if its window loses focus and gets it back while it runs. The themes example's self-test has the same risk with the appearance.

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
- **Where it stands, on 2026-11-26:** all twelve steps are done, and criteria 2 to 5 are met by tests that pass on Windows. Criterion 1 needs the checks by hand above, on each system, and they have not been made. On Windows they would change this machine's settings, and macOS and GNOME need machines of their own. CI has not yet run the M18 work. The milestone stays In progress until both are done.

## Done when

Copied from the overview.

1. Changing less motion, the text scale, the caret blink and the double-click time on the system changes a running application, on Windows, Linux and macOS.
2. A component that reads a setting is built again when it changes, and components that do not read it are not, checked by a test.
3. An animated value reaches its end at the time asked, within one frame, and no frames are drawn when nothing is animating.
4. With less motion set, transitions jump to their end and animated images pause.
5. The example's self-test checks each setting and the animations.
