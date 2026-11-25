//! Compile-fail tests: each file in `tests/fail` must not compile, and its
//! error must match the `.stderr` file next to it (M03 "Done when" item 4).
//!
//! After a deliberate change to a message, regenerate the files with
//! `TRYBUILD=overwrite cargo test -p dovetail-macros --test compile_fail`
//! and review every difference.

#[test]
fn mistakes_give_errors_at_the_declaration() {
  trybuild::TestCases::new().compile_fail("tests/fail/*.rs");
}
