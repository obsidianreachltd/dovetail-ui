//! Unicode's own grapheme break conformance test (GraphemeBreakTest.txt),
//! as generated into `tests/data` by `tools/ucd-gen`. Every case must pass.

use dovetail_text::{UNICODE_VERSION, grapheme_boundaries};

#[test]
fn every_unicode_grapheme_break_case_passes() {
  let data = include_str!("data/grapheme_break_test.txt");
  let mut cases = 0;
  let mut failures = Vec::new();
  for line in data
    .lines()
    .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
  {
    // "÷ 0020 × 0308 ÷": ÷ is a boundary, × is not.
    let mut text = String::new();
    let mut expected = Vec::new();
    for token in line.split_whitespace() {
      match token {
        "÷" => expected.push(text.len()),
        "×" => {},
        hex => text.push(
          char::from_u32(u32::from_str_radix(hex, 16).expect("hex"))
            .expect("char"),
        ),
      }
    }
    cases += 1;
    let found = grapheme_boundaries(&text);
    if found != expected {
      failures
        .push(format!("{line}\n  expected {expected:?}, found {found:?}"));
    }
  }
  assert!(cases > 700, "only {cases} cases read");
  assert!(
    failures.is_empty(),
    "{} of {cases} cases fail (Unicode {UNICODE_VERSION}):\n{}",
    failures.len(),
    failures[..failures.len().min(10)].join("\n")
  );
}
