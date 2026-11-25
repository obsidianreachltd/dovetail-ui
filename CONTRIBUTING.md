# Contributing to Dovetail UI

Thank you for helping. This guide explains how to set up, how work is organised, and how to write changes and documents. AI coding agents also follow [AGENTS.md](AGENTS.md).

## Setup

1. Install Rust with [rustup](https://rustup.rs). You need Rust 1.89 or later.
2. Add the tools: `rustup component add rustfmt clippy`.
3. Clone the repository and run `cargo build`.

Dovetail supports Windows, Linux and macOS. On Linux, test with both X11 and Wayland when your change affects windows or input.

## How work is organised

Work is divided into milestones. The [completion matrix](docs/milestones/README.md) shows where each one stands.

1. **Overview:** every milestone has a short overview of what it is about and when it is done.
2. **Plan:** before work starts, write a plan from the [plan template](docs/milestones/plans/_template.md) and set the status to Planned.
3. **Work:** set the status to In progress. Tick off steps in the plan as they are finished.
4. **Report:** when every "Done when" criterion is met, write a report from the [report template](docs/milestones/reports/_template.md) and set the status to Done.

Keep the status the same in the overview, the plan, the report and the matrix.

## Making a change

1. Create a branch from `main`, named after the milestone or bug, such as `m01-device-loss` or `b0003-ime-crash`.
2. Keep each commit to one change, with a short first line in the present tense ("Add surface resize handling").
3. Before you open a pull request, run:

   ```sh
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```

4. Open a pull request against `main`. Say which milestone step or bug it covers, and update the related documents in the same pull request.
5. CI must pass on Windows, Linux and macOS before it is merged.

## Other tests and tools

Some tests are left out of `cargo test --workspace`, because they need a GPU, change something on your machine, or need files that are not committed.

- **Examples' self-tests:** `cargo run -p example-<name> -- --self-test` drives the example's window by itself and exits with a failure code if a check fails. Run them all after a change to windows, layout or painting.
- **GPU tests** are `#[ignore]`d: `cargo test -p dovetail-render --all-features -- --ignored`. The sharpness test also needs the system fonts.
- **A picture of the painter's output:** `cargo run -p dovetail-render --example snapshot -- out.png 2`.
- **The clipboard test** is `#[ignore]`d because it changes the real clipboard: `cargo test -p dovetail-desktop -- --ignored clipboard`.
- **Accessibility checks on Windows**, through UI Automation, are in `tools/access-check/`.
- **Macro error messages:** after a deliberate change, run `TRYBUILD=overwrite cargo test -p dovetail-macros --test compile_fail`, and review every `.stderr` difference.
- **Platform code for other systems:** type-check it with `cargo clippy -p dovetail-desktop --target aarch64-apple-darwin -- -D warnings` and `--target x86_64-unknown-linux-gnu`. Add the targets first with `rustup target add`.
- **Unicode tables:** download the UCD 17.0.0 files into `tools/ucd/data/17.0.0/`, then run `cargo run -p ucd-gen -- 17.0.0` ([D0014](docs/decisions/D0014-unicode-data.md)). The bidirectional conformance tests also read the whole UCD test files from there when they are present.
- **Test files that are not committed**, in git-ignored folders. The tests that use them are skipped when the files are missing:
  - `tools/fonts/`: two Adobe CFF fonts for `crates/dovetail-text/tests/cff_fonts.rs` (the links are in the [M06 plan](docs/milestones/plans/M06-text-and-input.md), step 3);
  - `tools/images/`: test images from the Go source tree, for the image decoder tests (see the [M17 plan](docs/milestones/plans/M17-icons-and-images.md)).

## Code style

- Format with `rustfmt` and follow `clippy`.
- Every public item has a documentation comment.
- Keep the [dependency rules](docs/architecture/crates.md#rules). `dovetail-core` depends on no other Dovetail crate.
- Adding a dependency beyond winit, wgpu and AccessKit needs a decision record first.

## Reporting a bug

Follow the steps in the [bug list](docs/bugs/README.md#reporting-a-bug).

## Proposing a decision

Follow the steps in the [decision list](docs/decisions/README.md#making-a-decision). Talk about it in a pull request before you mark it as Accepted.

## Writing documents

- Start from the template for that kind of document, and keep its headings and their order.
- Write in British English, with plain words and short sentences.
- Use technical terms only when they are needed, and add new ones to the [glossary](docs/glossary.md).
- See the full [writing style](docs/README.md#writing-style).

## Licence

Dovetail UI is an open-source project owned by Obsidian Reach Ltd. It is licensed under MIT OR Apache-2.0 ([D0005](docs/decisions/D0005-permissive-licence.md)). Anyone may contribute.

Unless you say otherwise, any contribution you submit for inclusion is licensed under MIT OR Apache-2.0, as defined in the Apache-2.0 licence, with no extra terms. You keep the copyright of your contribution. Obsidian Reach Ltd and every other user receive it under those licences.

Do not add a separate copyright header to source files. The copyright for the project is in [NOTICE](NOTICE) and [LICENSE-MIT](LICENSE-MIT).
