# Goals

Dovetail UI is a Rust framework for building polished desktop applications on Windows, Linux and macOS.

## Purpose

The framework is the product (see [D0001](../decisions/D0001-framework-is-the-product.md)). Other developers should be able to build serious desktop applications of every kind with it: tools and utilities, dashboards, creative and engineering software, and applications that open and save documents.

Document-based editors are the hardest case, not the only one. Their needs, such as a GPU canvas, docking panels, undo and documents, are in packages of their own, so simpler applications leave them out.

The most demanding application in mind is a printed circuit board (PCB) design tool with the ambition of Altium. A small PCB-like editor is built alongside the framework to prove that it works (see [R01](../milestones/overviews/R01-first-release.md)). That editor is a test bed, not the product.

## Names

- Project name: Dovetail UI.
- Command and crate prefix: `dovetail`.
- Intended website: `dovetailui.dev`.
- Whether these names are free on crates.io, and whether the domain is free, still needs checking (milestone [M00](../milestones/overviews/M00-workspace-foundation.md)).

## Developer experience

A developer should be able to create and run an application from the command line:

```sh
dovetail new pcb-studio
dovetail dev
dovetail build --release
```

- `dovetail new` creates a small application that works straight away and shows clearly where to add things.
- `dovetail dev` runs the application while you work on it.
- `dovetail build` makes builds for a platform, and later packages them for installing.

Developers add packages of widgets and capabilities only when they need them.

Release builds run on each target operating system through continuous integration (CI). Building for all three systems from one machine is a separate capability, to be looked at after the first release.

## Fast iteration

Changes should be quick to see. The first aim is a reliable rebuild-and-restart loop. Live updates of UI layout, styling and assets without a restart come later, if they prove possible (see [Architecture](../architecture/overview.md#development-iteration)).

## First release and later

The first release ([R01](../milestones/overviews/R01-first-release.md)) is kept small enough to build and test properly. It comes after more milestones than first planned, so it may gain more of the items below. It includes:

- the command line tool, with `new`, `dev` and `build`
- a window with a declarative component tree
- reactive state and typed events
- layout, painting, text input and accessibility
- a custom GPU canvas
- a dockable inspector panel
- commands, shortcuts and undo
- saving and reopening documents
- Arabic and text that mixes directions
- theming, with light and dark themes
- everyday controls: scrollbars, check boxes, radio buttons, drop-downs, number fields, sliders, tooltips and context menus
- virtualised lists, trees and tables, and a property grid
- icons and images
- system settings such as less motion and text size, and animation that follows them
- several windows that remember their size and place
- tools for testing an application's interface
- files dropped on a window, opened from the system, and recent files
- translated text, local number and date formats, and mirrored layouts
- builds on Windows, Linux and macOS

Later capabilities include:

- live UI updates without a restart
- building for all platforms from one machine
- a larger widget catalogue, such as date pickers, colour pickers and charts
- notifications, a tray icon, and progress on the taskbar or dock ([M23](../milestones/overviews/M23-notifications-and-tray.md))
- printing and saving as PDF ([M24](../milestones/overviews/M24-printing.md))
