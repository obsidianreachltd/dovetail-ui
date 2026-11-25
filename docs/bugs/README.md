# Bugs

Each bug has its own file, `B0001-short-name.md`, written from the [template](_template.md). Numbers are never reused.

## Bug list

| ID | Summary | Status | Severity | Milestone | Platform |
|---|---|---|---|---|---|
| [B0001](B0001-vulkan-close-hangs-after-device-loss.md) | Closing a window hangs after a GPU device loss (Vulkan) | Fixed | High | M01 | Windows |
| [B0002](B0002-dx12-panic-after-device-loss.md) | wgpu panics after GPU device recovery (DX12) | Fixed | High | M01 | Windows |
| [B0003](B0003-held-key-repeats-across-new-windows.md) | A held key acts again in every new window | Fixed | Medium | M01 | Windows |
| [B0004](B0004-doubled-letter-when-typing.md) | A typed letter appeared twice once, and could not be repeated | Closed | Medium | M06 | Windows |
| [B0005](B0005-second-window-loses-rebuilds.md) | A second UI window loses the other window's rebuilds | Fixed | High | M10 | All |
| [B0006](B0006-text-field-right-walk-flaky.md) | The text field self-test's Right walk sometimes ends early | Open | Low | M13 | Windows |

## Status words

| Status | Meaning |
|---|---|
| Open | Reported, not yet being fixed. |
| In progress | Someone is fixing it. |
| Fixed | The fix is merged and a test covers it. |
| Closed | Not fixed (cannot be reproduced, a duplicate, or works as intended). The Notes section says why. |

## Severity words

| Severity | Meaning |
|---|---|
| Critical | Crash, lost data, or a broken build on a supported platform. |
| High | A main feature does not work, and there is no workaround. |
| Medium | A feature works badly, or there is a workaround. |
| Low | A small or cosmetic problem. |

## Reporting a bug

1. Check the list above for the same bug.
2. Copy the template to a new file with the next number.
3. Fill in every section, and keep the steps to reproduce as short as possible.
4. Add a row to the list above.
5. When it is fixed, fill in the Fix section and change the status in both places.
