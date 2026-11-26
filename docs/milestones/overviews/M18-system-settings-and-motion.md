# M18 — System settings and motion

| Field | Value |
|---|---|
| ID | M18 |
| Status | Planned |
| Depends on | M07, M14, M17 |
| Plan | [M18 plan](../plans/M18-system-settings-and-motion.md) |
| Report | — |

## Summary

Read the system settings that shape how an application should behave, such as less motion, text size, caret blink and double-click time, and add a small animation system that respects them.

## Why it matters

- **Settings that are ignored today:**
  - the caret blinks at a fixed rate;
  - double-clicks use a fixed time;
  - text does not follow the system's text size.
- **Motion:** `Image` plays animations ([M17](M17-icons-and-images.md)), but Dovetail cannot tell when the system asks for less motion. Applications must offer their own pause button.
- **Animation:** controls change state at once. There is no way to fade, slide or ease between values, and M14 left animating between themes until an animation system exists.
- **Accessibility:** less motion and larger text are accessibility settings. They belong with [M07](M07-accessibility.md)'s work, and its plan recorded less motion as a follow-up.
- **Later work:** every control and application that animates uses this.

## What it includes

- **System settings**, in `dovetail-core`, read on each system and updated while the application runs:
  - less motion (Windows' animation setting, macOS's Reduce Motion, GNOME's enable-animations);
  - the text scale (Windows' text size, GNOME's text scaling factor; macOS has none for applications);
  - the caret blink time, or no blinking;
  - the double-click time and distance;
  - the scroll-bar setting on macOS (always shown, or only while scrolling), left from M15.
- **Reading them in components:** one value passed down the tree, as the theme is, so only the components that read a setting are built again when it changes.
- **An animation system:**
  - an animated value that eases from one value to another over a time, with a few standard curves;
  - transitions for colours, sizes, positions and opacity;
  - driven by the window's frames, and no frames are drawn when nothing moves;
  - with less motion set, animations jump to their end, and `Image` animations pause unless the application asks otherwise.
- **Controls that use it:** a few small transitions, such as a check box's tick, a toggle's knob, a collapsible's opening and the theme changing.
- **The text scale** applied by the theme to every text style.
- **An example** with a `--self-test` that sets each setting from inside the program and checks the result.

## What it leaves out

- Physics-based motion, such as springs and flinging a scrolled list.
- Animation along paths, and key-frame timelines.
- Controls' native looks on each system.

## Done when

1. Changing less motion, the text scale, the caret blink and the double-click time on the system changes a running application, on Windows, Linux and macOS.
2. A component that reads a setting is built again when it changes, and components that do not read it are not, checked by a test.
3. An animated value reaches its end at the time asked, within one frame, and no frames are drawn when nothing is animating.
4. With less motion set, transitions jump to their end and animated images pause.
5. The example's self-test checks each setting and the animations.
