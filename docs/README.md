# Documentation

This folder holds all project documents. Start with [goals](vision/goals.md), then the [architecture overview](architecture/overview.md), then the [milestones](milestones/README.md).

## Contents

| Folder or file | What it holds |
|---|---|
| [vision/goals.md](vision/goals.md) | What we are building, who it is for, and what the first release covers |
| [vision/principles.md](vision/principles.md) | Rules that guide design choices |
| [architecture/overview.md](architecture/overview.md) | The layers, component model, rendering, development loop, repository layout, testing and risks |
| [architecture/crates.md](architecture/crates.md) | The packages and how they depend on each other |
| [architecture/syntax.md](architecture/syntax.md) | The declarative `ui!` syntax and its open questions |
| [architecture/testing.md](architecture/testing.md) | How to test a user interface with `dovetail-testing`: headless, pictures and real windows |
| [architecture/reactive-handles-prototype.md](architecture/reactive-handles-prototype.md) | M02 prototype report comparing two designs for reactive state handles |
| [architecture/window-tests-prototype.md](architecture/window-tests-prototype.md) | M20 prototype report: real-window tests run from `cargo test` |
| [decisions/](decisions/README.md) | Records of important decisions |
| [milestones/](milestones/README.md) | Milestone overviews, plans and completion reports, with the completion matrix |
| [bugs/](bugs/README.md) | Bug records |
| [glossary.md](glossary.md) | Plain meanings of terms |

## Document kinds

Every document of one kind follows its template exactly. It has the same headings, in the same order, and a details table at the top.

| Kind | Template | File name |
|---|---|---|
| Milestone overview | [template](milestones/overviews/_template.md) | `M00-short-name.md` |
| Milestone plan | [template](milestones/plans/_template.md) | Same name as its overview |
| Completion report | [template](milestones/reports/_template.md) | Same name as its overview |
| Decision | [template](decisions/_template.md) | `D0001-short-name.md` |
| Bug | [template](bugs/_template.md) | `B0001-short-name.md` |

## Writing style

- Write in British English (behaviour, licence, organise).
- Use short sentences and plain words. Use technical terms only when they are needed, and add new ones to the [glossary](glossary.md).
- Put one idea in each list item.
- Write dates as YYYY-MM-DD.
- Use relative links between documents.
- Put code, commands, crate names and file names in `backticks`.
