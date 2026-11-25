//! The bidirectional algorithm against Unicode's conformance tests
//! (M13, D0018): `BidiTest.txt` gives classes, `BidiCharacterTest.txt`
//! gives characters. A sample of each is committed in `tests/data`; the
//! whole files are used when they are in `tools/ucd/data/<version>/`.

use std::path::PathBuf;

use dovetail_text::{
  BidiClass, UNICODE_VERSION, bidi_class, bracket_key, resolve_bidi,
  visual_order,
};

fn class(name: &str) -> BidiClass {
  use BidiClass::*;
  match name {
    "L" => L,
    "R" => R,
    "AL" => Al,
    "EN" => En,
    "ES" => Es,
    "ET" => Et,
    "AN" => An,
    "CS" => Cs,
    "NSM" => Nsm,
    "BN" => Bn,
    "B" => B,
    "S" => S,
    "WS" => Ws,
    "ON" => On,
    "LRE" => Lre,
    "LRO" => Lro,
    "RLE" => Rle,
    "RLO" => Rlo,
    "PDF" => Pdf,
    "LRI" => Lri,
    "RLI" => Rli,
    "FSI" => Fsi,
    "PDI" => Pdi,
    other => panic!("unknown class {other}"),
  }
}

/// The levels as the test files write them: `x` for removed characters.
fn written(levels: &[Option<u8>]) -> String {
  levels
    .iter()
    .map(|l| l.map_or_else(|| "x".to_owned(), |l| l.to_string()))
    .collect::<Vec<_>>()
    .join(" ")
}

/// The visual order of the characters that are not removed.
fn order(levels: &[Option<u8>]) -> String {
  let kept: Vec<usize> = (0..levels.len())
    .filter(|&i| levels[i].is_some())
    .collect();
  let only: Vec<u8> = kept.iter().map(|&i| levels[i].unwrap()).collect();
  visual_order(&only)
    .into_iter()
    .map(|k| kept[k].to_string())
    .collect::<Vec<_>>()
    .join(" ")
}

fn full(name: &str) -> Option<String> {
  let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../../tools/ucd/data")
    .join(UNICODE_VERSION)
    .join(name);
  std::fs::read_to_string(path).ok()
}

fn sample(name: &str) -> String {
  let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("tests/data")
    .join(name);
  std::fs::read_to_string(path).expect("the committed sample")
}

/// Runs every case of a BidiTest file; returns the number run, and the
/// first failures.
fn bidi_test(text: &str) -> (usize, Vec<String>) {
  let (mut levels, mut reorder) = (String::new(), String::new());
  let mut run = 0;
  let mut failures = Vec::new();
  for line in text.lines() {
    let line = line.split('#').next().unwrap().trim();
    if line.is_empty() {
      continue;
    }
    if let Some(l) = line.strip_prefix("@Levels:") {
      levels = l.trim().to_owned();
      continue;
    }
    if let Some(r) = line.strip_prefix("@Reorder:") {
      reorder = r.trim().to_owned();
      continue;
    }
    if line.starts_with('@') {
      continue;
    }
    let (input, bits) = line.split_once(';').unwrap();
    let classes: Vec<BidiClass> = input.split_whitespace().map(class).collect();
    let none = vec![None; classes.len()];
    let bits: u8 = bits.trim().parse().unwrap();
    for (bit, para) in [(1, None), (2, Some(0)), (4, Some(1))] {
      if bits & bit == 0 {
        continue;
      }
      run += 1;
      let (_, got) = resolve_bidi(&classes, &none, para);
      let (got_levels, got_order) = (written(&got), order(&got));
      if (got_levels != levels || got_order != reorder) && failures.len() < 10 {
        failures.push(format!(
          "{input} (para {para:?}): levels {got_levels} want {levels}; order {got_order} want {reorder}"
        ));
      }
    }
  }
  (run, failures)
}

fn character_test(text: &str) -> (usize, Vec<String>) {
  let mut run = 0;
  let mut failures = Vec::new();
  for line in text.lines() {
    let line = line.split('#').next().unwrap().trim();
    if line.is_empty() {
      continue;
    }
    let f: Vec<&str> = line.split(';').collect();
    let chars: Vec<char> = f[0]
      .split_whitespace()
      .map(|h| char::from_u32(u32::from_str_radix(h, 16).unwrap()).unwrap())
      .collect();
    let para = match f[1] {
      "0" => Some(0),
      "1" => Some(1),
      _ => None,
    };
    let classes: Vec<BidiClass> =
      chars.iter().map(|&c| bidi_class(c)).collect();
    let brackets: Vec<_> = chars.iter().map(|&c| bracket_key(c)).collect();
    run += 1;
    let (level, got) = resolve_bidi(&classes, &brackets, para);
    let (got_levels, got_order) = (written(&got), order(&got));
    let want_level: u8 = f[2].parse().unwrap();
    if (level != want_level
      || got_levels != f[3].trim()
      || got_order != f[4].trim())
      && failures.len() < 10
    {
      failures.push(format!(
        "{}: para {level} want {want_level}; levels {got_levels} want {}; order {got_order} want {}",
        f[0], f[3], f[4]
      ));
    }
  }
  (run, failures)
}

#[test]
fn bidi_test_cases_pass() {
  let (run, failures) = bidi_test(&sample("bidi_test_sample.txt"));
  assert!(run > 10_000, "{run} cases");
  assert!(failures.is_empty(), "{run} cases:\n{}", failures.join("\n"));
  match full("BidiTest.txt") {
    Some(text) => {
      let (run, failures) = bidi_test(&text);
      println!("BidiTest.txt: {run} cases");
      assert!(failures.is_empty(), "{run} cases:\n{}", failures.join("\n"));
    },
    None => println!("BidiTest.txt is not downloaded; the sample passed"),
  }
}

#[test]
fn bidi_character_test_cases_pass() {
  let (run, failures) =
    character_test(&sample("bidi_character_test_sample.txt"));
  assert!(run > 1_000, "{run} cases");
  assert!(failures.is_empty(), "{run} cases:\n{}", failures.join("\n"));
  match full("BidiCharacterTest.txt") {
    Some(text) => {
      let (run, failures) = character_test(&text);
      println!("BidiCharacterTest.txt: {run} cases");
      assert!(failures.is_empty(), "{run} cases:\n{}", failures.join("\n"));
    },
    None => {
      println!("BidiCharacterTest.txt is not downloaded; the sample passed")
    },
  }
}
