# D0020 — The default theme: deep slate, smaller text, bold buttons

| Field | Value |
|---|---|
| ID | D0020 |
| Status | Accepted |
| Date | 2026-11-23 |

## Context

[D0019](D0019-theming-foundations.md) made the default theme from Dovetail's bright blue (#1E66F5), with 16-pixel text. Seen in the themes example, the bright blue looked dated, and the text and buttons did not stand out the way tools for serious work should. This record replaces only D0019's choice of the default colour. The rest of D0019 stands.

## Decision

- **The default seed is a deep slate blue, #33415C** (`DOVETAIL_SLATE`).
- **The default light theme keeps the seed itself as its primary colour** (`ColorScheme::from_seed_fidelity`), as Flutter's fidelity variant does, so buttons are the deep slate and not a brighter tone made from it. The dark theme takes the usual tones, so its buttons are a light slate.
- **Muted seeds stay muted.** `from_seed` no longer raises the primary colour's chroma to a minimum, so a quiet seed gives a quiet scheme.
- **Text is smaller:** body text is 14 pixels (it was 16), and so is layout's `DEFAULT_FONT_SIZE`. Titles are 15, small text 12 and headlines 22.
- **Button labels are bold** (700), at 14 pixels, so buttons stand out.
- **Text fields take their size from the theme's body text** and set it on their own element, so what they measure for the caret is what they show.

## Consequences

- The default look is calmer and darker, and surfaces are a neutral grey rather than blue-white.
- Every text role in every scheme is still checked for WCAG AA contrast, fidelity schemes included.
- Applications that want the old look seed their theme with #1E66F5.

## Alternatives considered

- **Deep indigo (#3730A3):** deeper, but still clearly coloured.
- **Near-neutral graphite (#3F4448):** almost no colour, leaving it to applications.
