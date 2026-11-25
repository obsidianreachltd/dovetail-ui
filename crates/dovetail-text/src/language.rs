//! Languages, as far as choosing fonts needs them (M06).
//!
//! Han characters (kanji, hanzi, hanja) are shared by Japanese, Chinese and
//! Korean, but each draws many of them differently. So for Han characters
//! and CJK punctuation, the font list prefers a font made for the text's
//! language. The language comes from the text itself when it has kana
//! (Japanese) or Hangul (Korean), and otherwise from the font list's
//! language, which the desktop crate sets from the system.

/// A language whose writing changes which CJK font to use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Language {
  /// Not known, or one that does not change the font.
  #[default]
  Other,
  /// Japanese.
  Japanese,
  /// Chinese in simplified characters (mainland China, Singapore).
  ChineseSimplified,
  /// Chinese in traditional characters (Taiwan, Hong Kong, Macau).
  ChineseTraditional,
  /// Korean.
  Korean,
}

impl Language {
  /// The language of a tag such as `ja`, `ja-JP`, `zh-Hans`, `zh_TW.UTF-8`
  /// or `ko-KR`. Tags for other languages give [`Language::Other`].
  pub fn from_tag(tag: &str) -> Language {
    let tag = tag
      .split(['.', '@'])
      .next()
      .unwrap_or("")
      .to_ascii_lowercase()
      .replace('_', "-");
    let mut parts = tag.split('-');
    match parts.next().unwrap_or("") {
      "ja" => Language::Japanese,
      "ko" => Language::Korean,
      "zh" => {
        let rest: Vec<&str> = parts.collect();
        let traditional = rest
          .iter()
          .any(|p| matches!(*p, "hant" | "tw" | "hk" | "mo"));
        if traditional {
          Language::ChineseTraditional
        } else {
          Language::ChineseSimplified
        }
      },
      _ => Language::Other,
    }
  }

  /// The bits of a font's `OS/2` code page ranges that mark it as made for
  /// this language: JIS (932), GB (936), Wansung or Johab (949, 1361), and
  /// Big5 (950).
  pub(crate) fn code_pages(self) -> u32 {
    match self {
      Language::Other => 0,
      Language::Japanese => 1 << 17,
      Language::ChineseSimplified => 1 << 18,
      Language::Korean => (1 << 19) | (1 << 21),
      Language::ChineseTraditional => 1 << 20,
    }
  }

  /// The language a family name says a font is for, by the region at its
  /// end, as in "Noto Sans CJK JP" or "Source Han Sans KR".
  pub(crate) fn of_family(family: &str) -> Option<Language> {
    match family.rsplit(' ').next()? {
      "JP" => Some(Language::Japanese),
      "KR" | "K" => Some(Language::Korean),
      "SC" => Some(Language::ChineseSimplified),
      "TC" | "HK" => Some(Language::ChineseTraditional),
      _ => None,
    }
  }

  /// The language `text` shows it is in: Japanese when it has kana, Korean
  /// when it has Hangul.
  pub(crate) fn of_text(text: &str) -> Option<Language> {
    text.chars().find_map(|c| match c as u32 {
      0x3040..=0x30ff | 0x31f0..=0x31ff | 0xff66..=0xff9f => {
        Some(Language::Japanese)
      },
      0x1100..=0x11ff | 0x3130..=0x318f | 0xac00..=0xd7af => {
        Some(Language::Korean)
      },
      _ => None,
    })
  }
}

/// True for characters whose shape depends on the CJK language: Han
/// characters, CJK punctuation and full-width forms.
pub(crate) fn varies_by_language(c: char) -> bool {
  matches!(
    c as u32,
    0x2e80..=0x2fdf     // radicals
      | 0x3000..=0x303f // CJK symbols and punctuation
      | 0x3400..=0x4dbf // extension A
      | 0x4e00..=0x9fff // unified ideographs
      | 0xf900..=0xfaff // compatibility ideographs
      | 0xff00..=0xffef // full-width forms
      | 0x20000..=0x3ffff // extensions B and later
  )
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn tags_from_systems_and_the_web_are_understood() {
    for (tag, want) in [
      ("ja", Language::Japanese),
      ("ja-JP", Language::Japanese),
      ("ja_JP.UTF-8", Language::Japanese),
      ("zh-CN", Language::ChineseSimplified),
      ("zh-Hans-SG", Language::ChineseSimplified),
      ("zh_TW.UTF-8", Language::ChineseTraditional),
      ("zh-Hant", Language::ChineseTraditional),
      ("zh-HK", Language::ChineseTraditional),
      ("ko-KR", Language::Korean),
      ("en-GB", Language::Other),
      ("C", Language::Other),
      ("", Language::Other),
    ] {
      assert_eq!(Language::from_tag(tag), want, "{tag}");
    }
  }

  #[test]
  fn kana_and_hangul_name_the_language() {
    assert_eq!(Language::of_text("日本です"), Some(Language::Japanese));
    assert_eq!(Language::of_text("カタカナ"), Some(Language::Japanese));
    assert_eq!(Language::of_text("한국어"), Some(Language::Korean));
    assert_eq!(Language::of_text("中文"), None, "Han alone could be any");
    assert!(varies_by_language('直'));
    assert!(varies_by_language('。'));
    assert!(!varies_by_language('A'));
    assert_eq!(
      Language::of_family("Noto Sans CJK JP"),
      Some(Language::Japanese)
    );
    assert_eq!(Language::of_family("Microsoft YaHei"), None);
  }
}
