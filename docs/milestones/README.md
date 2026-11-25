# Milestones

Each milestone has up to three files with the same name in three folders:

- [overviews](overviews/): what the milestone is about. Every milestone has one.
- [plans](plans/): how it will be done. Written when the milestone is about to start.
- [reports](reports/): what was done and whether it met its criteria. Written when it finishes.

Use the `_template.md` file in each folder for a new document.

## Completion matrix

| ID | Milestone | Status | Depends on | Overview | Plan | Report | Criteria met |
|---|---|---|---|---|---|---|---|
| M00 | Workspace foundation | Done | — | [Overview](overviews/M00-workspace-foundation.md) | [Plan](plans/M00-workspace-foundation.md) | [Report](reports/M00-workspace-foundation.md) | 6/6 |
| M01 | Window and GPU | In progress | M00 | [Overview](overviews/M01-window-and-gpu.md) | [Plan](plans/M01-window-and-gpu.md) | — | 5/6 |
| M02 | Component and state core | Done | M00 | [Overview](overviews/M02-component-and-state-core.md) | [Plan](plans/M02-component-and-state-core.md) | [Report](reports/M02-component-and-state-core.md) | 6/6 |
| M03 | Declarative macro | Done | M02 | [Overview](overviews/M03-declarative-macro.md) | [Plan](plans/M03-declarative-macro.md) | [Report](reports/M03-declarative-macro.md) | 5/5 |
| M04 | Layout and painting | Done | M01, M03 | [Overview](overviews/M04-layout-and-painting.md) | [Plan](plans/M04-layout-and-painting.md) | [Report](reports/M04-layout-and-painting.md) | 6/6 |
| M05 | CLI and rebuild loop | Done | M04 | [Overview](overviews/M05-cli-and-rebuild-loop.md) | [Plan](plans/M05-cli-and-rebuild-loop.md) | [Report](reports/M05-cli-and-rebuild-loop.md) | 5/5 |
| M06 | Text and input | In progress | M04 | [Overview](overviews/M06-text-and-input.md) | [Plan](plans/M06-text-and-input.md) | [Report](reports/M06-text-and-input.md) | 6/7 |
| M07 | Accessibility | In progress | M06 | [Overview](overviews/M07-accessibility.md) | [Plan](plans/M07-accessibility.md) | — | 4/5 |
| M08 | Commands and undo | In progress | M06 | [Overview](overviews/M08-commands-and-undo.md) | [Plan](plans/M08-commands-and-undo.md) | — | 4/5 |
| M09 | Custom GPU canvas | In progress | M06 | [Overview](overviews/M09-custom-gpu-canvas.md) | [Plan](plans/M09-custom-gpu-canvas.md) | — | 5/5 |
| M10 | Docking | In progress | M08 | [Overview](overviews/M10-docking.md) | [Plan](plans/M10-docking.md) | — | 5/5 |
| M11 | Documents | In progress | M08 | [Overview](overviews/M11-documents.md) | [Plan](plans/M11-documents.md) | — | 5/5 |
| M13 | Complex scripts | In progress | M06 | [Overview](overviews/M13-complex-scripts.md) | [Plan](plans/M13-complex-scripts.md) | — | 4/4 |
| M14 | Theming | In progress | M06, M10, M11 | [Overview](overviews/M14-theming.md) | [Plan](plans/M14-theming.md) | — | 5/6 |
| M15 | Everyday controls | In progress | M07, M08, M14 | [Overview](overviews/M15-everyday-controls.md) | [Plan](plans/M15-everyday-controls.md) | — | 6/6 |
| M16 | Data views | In progress | M15 | [Overview](overviews/M16-data-views.md) | [Plan](plans/M16-data-views.md) | — | 6/6 |
| M17 | Icons and images | In progress | M14 | [Overview](overviews/M17-icons-and-images.md) | [Plan](plans/M17-icons-and-images.md) | — | 8/8 |
| M18 | System settings and motion | Not started | M07, M14, M17 | [Overview](overviews/M18-system-settings-and-motion.md) | — | — | 0/5 |
| M19 | Window management | Not started | M10, M11 | [Overview](overviews/M19-window-management.md) | — | — | 0/6 |
| M20 | Testing tools | In progress | M07, M15 | [Overview](overviews/M20-testing-tools.md) | [Plan](plans/M20-testing-tools.md) | — | 0/6 |
| M21 | Files from the system | Not started | M11, M19 | [Overview](overviews/M21-files-from-the-system.md) | — | — | 0/5 |
| M22 | Localisation | Not started | M13, M15, M16 | [Overview](overviews/M22-localisation.md) | — | — | 0/6 |
| R01 | First release | Not started | M05, M07, M09, M10, M11, M13, M14, M15, M16, M17, M18, M19, M20, M21, M22 | [Overview](overviews/R01-first-release.md) | — | — | 0/6 |
| M23 | Notifications and the tray | Not started | R01 | [Overview](overviews/M23-notifications-and-tray.md) | — | — | 0/5 |
| M24 | Printing | Not started | R01 | [Overview](overviews/M24-printing.md) | — | — | 0/5 |

There are two kinds of milestone:

- **Mnn** milestones build part of the framework.
- **Rnn** milestones are releases. They gather finished milestones, package them and publish them. R01, the first release, was called M12 until 2026-11-23. It comes after more feature milestones than it did then, so new Mnn milestones may be added before it.

IDs are not the order of work: "Depends on" is. M13 was split from M06 on 2026-11-20 and comes after it, before the first release. M18 to M24 were added on 2026-11-25: M18 to M22 come before the first release, and M23 and M24 after it. On the same day CI first passed on all three systems, which finished M00 and M04 and met five of M01's six criteria. M01's last one, moving between displays with different scales, needs a person.

## Status words

| Status | Meaning |
|---|---|
| Not started | Only the overview exists. |
| Planned | The plan is written, but work has not begun. |
| In progress | Work has begun. |
| Done | The report is written and every criterion is met. |
| Dropped | The milestone will not be done. Its overview says why. |

## Updating the matrix

- Change a row whenever you change that milestone's status. Use the same status in the overview, the plan and the matrix.
- When a plan is added, link it and set the status to Planned.
- When work begins, set the status to In progress.
- Update "Criteria met" as each "Done when" item is met.
- When the report is added, link it and set the status to Done. A milestone is Done only when every criterion is met.
- `—` means the document does not exist yet.
