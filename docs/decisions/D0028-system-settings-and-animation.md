# D0028 — System settings in core, animation on frames, and opacity and offsets in layout

| Field | Value |
|---|---|
| ID | D0028 |
| Status | Accepted |
| Date | 2026-11-26 |

## Context

M18 reads the system settings that shape how an application behaves, and adds a small animation system that respects them ([M18 plan](../milestones/plans/M18-system-settings-and-motion.md), step 1). Four questions must be settled first:

- **Settings:** less motion, the text scale, the caret blink, the double-click time and distance, and overlay scroll bars. Where are they kept, and how does a component read one without being built again when another changes?
- **Frames:** nothing can ask for the next frame today. The desktop draws when something changed, and wakes at the next timer or `wake_after`. `Image` plays animations with a `Task::after` for each frame's delay.
- **Fades and slides:** `Style` has sizes, padding, margins and colours, but no opacity and no offset from the laid-out place.
- **The text scale:** text sizes come from the theme's text styles, from the body style the outermost `Theme` gives, and from sizes an application sets itself.

The system's appearance (M14) already solves the first question for dark mode, the accent colour and high contrast. [D0027](D0027-testing-foundations.md) gave the session its settings and put the clock and timers in core.

## Decision

### Settings: `SystemSettings` in `dovetail-core`, with readers for each

- **`SystemSettings`** sits beside `SystemAppearance`, in `crates/dovetail-core/src/settings.rs`:
  - `reduce_motion: bool`;
  - `text_scale: f32`, kept between 0.5 and 3;
  - `caret_blink: Option<Duration>`, `None` when blinking is off;
  - `double_click_time: Duration` and `double_click_distance: f32`, in logical pixels;
  - `overlay_scrollbars: bool`.
- **A function for each**, such as `reduce_motion()` and `text_scale()`, and `system_settings()` for all of them. Read while a component builds, each remembers that component as its reader, as `system_appearance()` does. Each setting keeps its own readers, so a component that reads only less motion is not built again when the caret blink changes.
- **`set_system_settings`** compares each setting with the one before, and builds again only the readers of those that changed.
- **Defaults**, where no desktop sets them, as in headless tests: motion on, a scale of 1, a 530 ms blink, 500 ms and 4 pixels for a double-click, and no overlay scroll bars. These are today's values.
- **The desktop reads them whenever it reads the appearance:** at start, on winit's `ThemeChanged`, and when a window comes to the front. Changing a setting in the system's settings window, then going back to the application, is seen. A change while the application stays in front is seen at its next focus. Each system's change notification, such as `WM_SETTINGCHANGE` on Windows, which winit does not pass on, is left until a need is found.
- **The session is still given its settings**, as D0027 says. The desktop gives each window's session new `Settings` when they change: the caret blink, the double-click time and distance, and overlay scroll bars.
- **Tests set them** with `set_system_settings`, or `Options` in `TestUi`, from inside the program. Nothing changes the real system's settings.

### Frames: requests in `dovetail-core`, answered by the window

- **`request_frame()`**, called while a component builds, asks for that component to be built again at the next frame. A component that wants frames asks again on each build, and stops asking when it is done.
- **Frames come from the display.** While any frame is asked for, the desktop asks winit for a redraw after each frame it shows, which the surface's vsync paces to the display's rate. At the start of each frame it builds the components that asked. When none asks, no frames are drawn.
- **The frame's time is `now()`.** An animation works out its value from the time, not by counting frames. So it ends at the time asked, within one frame, at any display rate and when frames are dropped.
- **In tests:** `TestUi::advance` steps through the time it is given at 60 frames a second, building the components that asked at each step, and fires timers in order between the steps. With no frame asked for, it moves straight to the next timer, as it does now. `TestWindows` waits for real frames.
- **Timers stay** for delays, such as the tooltip's, and for animated images, whose frames have their own delays.

### Animated values: `use_animated` in `dovetail-core`

- **`use_animated(target, transition)`** gives the value to show now. When `target` changes, the value eases from the value shown to the new target over the transition's time. It asks for frames until it gets there.
- **A new target part of the way through** starts from the value shown, with the full time again, so a toggle pressed twice quickly turns back smoothly.
- **`Transition`** has a duration and a curve: `Linear`, `EaseIn`, `EaseOut` and `EaseInOut`, the cubic curves CSS uses. `Transition::NONE` jumps.
- **The `Animate` trait** says how a value goes from one value to another. Dovetail gives it for `f32`, `Color` (in linear light, so colours do not go dark half way), `Edges`, and pairs and arrays of these. Applications add it for their own types.
- **With less motion set,** every `use_animated` value jumps to its target. It reads `reduce_motion()`, so animating components are built again when the setting changes. Animations that carry meaning, such as a progress bar, should change their values without a transition, so they still show.
- **Animated images stop too.** With less motion set, an animated `Image` shows its first frame and does not play. `always_play: true` plays it anyway, for an image that is the point of the screen, such as a preview.
- **Controls take a `Transition`** from the theme: a `motion` group in `ThemeData` with short and medium durations, as Material and Fluent have.

### Opacity and offsets: in `Style`, worked out by layout

- **`Style::offset`**, a pair of logical pixels, moves an element and everything in it from the place layout gives it, without moving anything else, as a CSS translate does. Layout applies it after placing the element, to the box's rectangle and clip and to those inside it. So painting, hit testing, the focus ring, IME areas and the accessibility bounds all follow it, with no change to them.
- **`Style::opacity`**, from 0 to 1, makes an element and everything in it see-through. Layout multiplies it down the tree, so each box carries its own opacity. The painter multiplies the alpha of everything it draws for a box by it: backgrounds, borders, text, icons, images, the caret and the selection. An element at opacity 0 is still laid out and can still be clicked, as in CSS. Hide it to stop that.
- **Children that overlap show through each other** while a group is see-through, because each draws at the group's opacity. The controls that fade in M18 have no overlapping children. Drawing a group into a texture first, which avoids this, is left until it is needed.
- **Canvases are not faded.** A canvas draws with its own wgpu pass, and the painter cannot change its alpha. Its offset still moves it, as its rectangle moves.

### The text scale: the theme's text, not sizes applications set

- **The outermost `Theme`** multiplies its text styles by `text_scale()`: `headline`, `title`, `body`, `body_small` and `label`. Every control reads them, so every control's text grows. The body style it gives to the text inside covers text with no size of its own.
- **A size an application sets itself**, such as `Text { size: Some(24.0) }`, is not scaled. The application chose it, perhaps to fit a drawing. It can multiply by `text_scale()`, or use a theme style.
- **Only text grows.** Boxes grow to fit their text, as they do now. Fixed sizes, icons and spacing do not change.

## Consequences

- **Settings follow the system while an application runs**, with no restart, and only the components that read a setting are built again.
- **Animations cost nothing when nothing moves:** no frames, no timers.
- **Animations are exact in tests.** `advance` gives the frames, so a test checks each one by moving the clock, and never waits.
- **Animated values work out where they are from the time**, so a slow frame makes an animation jump ahead, not run late.
- **Fades and slides need no change in painting's order, hit testing or accessibility**, because layout moves and fades the boxes. Layout gains two fields in `Style` and one in each box.
- **A see-through group with overlapping children looks wrong.** It is documented on `Style::opacity`.
- **A theme that fades** builds every component that reads the theme again on each frame, which is most of the window. M18 measures it (plan, Risks).
- **`dovetail-core` still has no dependencies.** Settings, frame requests and animated values use only the standard library.
- **New code must animate with `use_animated`**, not its own timers, so that less motion and tests reach it. AGENTS.md gains a line for this.

## Alternatives considered

- **One reader list for all settings**, as the appearance has. Simpler, but a component that reads less motion would be built again whenever the caret blink changed, which criterion 2 rules out.
- **Settings in the theme.** Components already read the theme. But the session and `Image` need settings with no theme around them, and a theme would be built again for every setting.
- **Reading settings in the session from core**, instead of the desktop giving them. Less to pass, but D0027 says the session is given its settings, and keeping that keeps the session easy to test alone.
- **Frames from timers at 60 a second.** It needs nothing new, but frames would not match the display: they would drift against vsync, and be too slow for 120 Hz displays.
- **Counting frames instead of reading the time.** Simpler sums, but an animation would run late whenever a frame was slow, and its length would depend on the display.
- **Offsets and opacity in the painter only.** Less work in layout, but clicks, the focus ring and screen readers would all use the place the element was laid out, not the place it is drawn.
- **Drawing see-through groups into a texture.** Correct where children overlap, but it needs a texture for each group on each frame, and the M18 controls do not need it.
- **Scaling every text size, including those applications set.** It is what some systems do, but it would change text an application has fitted to a drawing or a canvas, with no way to opt out.
