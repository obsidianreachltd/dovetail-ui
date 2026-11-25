# Glossary

Plain meanings of the terms used in these documents.

| Term | Meaning |
|---|---|
| AccessKit | A Rust library that connects an application's accessibility tree to each operating system's accessibility services. |
| Accessibility tree | A description of the UI (roles, names, states, actions) that assistive tools such as screen readers read. |
| Component | A reusable part of a UI, such as a button or a panel. It has properties, events, state and children. |
| Crate | A Rust package. Dovetail is split into several crates. |
| Declarative UI | Describing what the UI should look like, and letting the framework work out how to update it. |
| Device loss | When the GPU device stops working (for example after a driver update) and must be created again. |
| Dirty state | A document has changes that are not saved yet. |
| DPI, display scale | How many physical pixels each logical pixel uses on a screen, for example 150%. |
| Event | A message that something happened, usually a user action such as a click. |
| Facade crate | The `dovetail-ui` package (used in code as `dovetail`), which re-exports the common parts so applications need only one import. |
| GPU | The graphics processor. Dovetail draws with it through wgpu. |
| Identity | What makes a component the same component between updates, so it keeps its state. |
| IME | Input method editor. A system tool for typing languages such as Chinese or Japanese, where several key presses build up one character. |
| Key | A stable value that tells the framework which item in a list is which, even when the list changes. |
| Procedural macro | Rust code that runs at compile time and writes other Rust code. `ui!` and `#[component]` are procedural macros. |
| Property | An input given to a component, such as a button's text. |
| Reactive state | A value that, when it changes, makes the parts of the UI that use it update. |
| Render pass | One step of GPU drawing into a frame. |
| Shaping | Turning text into the right glyphs and positions. Needed for scripts such as Arabic. |
| Surface | The area of a window that the GPU draws into. |
| Validation app | The small PCB-like document editor built to prove that the framework works. |
| wgpu | A Rust library for drawing with the GPU on all three operating systems. |
| winit | A Rust library for creating windows and receiving input on all three operating systems. |
| Workspace | A set of crates in one repository that are built together by Cargo. |
