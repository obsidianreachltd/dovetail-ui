//! A small XML reader, enough for SVG icons: elements and attributes.
//! Text, comments, processing instructions, `DOCTYPE` and `CDATA` are
//! skipped.

/// An element with its attributes and child elements.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Node {
  /// Its name, without any namespace prefix.
  pub name: String,
  /// Its attributes in order, names without a namespace prefix.
  pub attributes: Vec<(String, String)>,
  /// The elements inside it.
  pub children: Vec<Node>,
}

impl Node {
  /// The value of attribute `name`.
  pub fn get(
    &self,
    name: &str,
  ) -> Option<&str> {
    self
      .attributes
      .iter()
      .find(|(n, _)| n == name)
      .map(|(_, v)| v.as_str())
  }
}

/// What was wrong with the XML, and where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlError {
  /// The byte where the problem was found.
  pub at: usize,
  /// What it was.
  pub what: &'static str,
}

/// Reads a document and returns its root element.
pub fn parse(text: &str) -> Result<Node, XmlError> {
  let mut p = Parser { s: text.as_bytes(), at: 0 };
  loop {
    p.skip_misc()?;
    if p.at >= p.s.len() {
      return Err(p.error("no root element"));
    }
    if p.s[p.at] == b'<' {
      return p.element();
    }
    p.at += 1;
  }
}

struct Parser<'a> {
  s: &'a [u8],
  at: usize,
}

impl Parser<'_> {
  fn error(
    &self,
    what: &'static str,
  ) -> XmlError {
    XmlError { at: self.at, what }
  }

  fn starts(
    &self,
    prefix: &str,
  ) -> bool {
    self.s[self.at..].starts_with(prefix.as_bytes())
  }

  fn skip_to(
    &mut self,
    end: &str,
  ) -> Result<(), XmlError> {
    while self.at < self.s.len() {
      if self.starts(end) {
        self.at += end.len();
        return Ok(());
      }
      self.at += 1;
    }
    Err(self.error("a comment or declaration that does not end"))
  }

  /// Skips text, comments, declarations and processing instructions, and
  /// stops at the next element tag, end tag or the end.
  fn skip_misc(&mut self) -> Result<(), XmlError> {
    while self.at < self.s.len() {
      if self.starts("<!--") {
        self.skip_to("-->")?;
      } else if self.starts("<![CDATA[") {
        self.skip_to("]]>")?;
      } else if self.starts("<?") {
        self.skip_to("?>")?;
      } else if self.starts("<!") {
        self.skip_declaration()?;
      } else if self.s[self.at] == b'<' {
        return Ok(());
      } else {
        self.at += 1;
      }
    }
    Ok(())
  }

  /// Skips `<!DOCTYPE ...>`, which may hold `[...]`.
  fn skip_declaration(&mut self) -> Result<(), XmlError> {
    let mut depth = 0;
    while self.at < self.s.len() {
      match self.s[self.at] {
        b'[' => depth += 1,
        b']' => depth -= 1,
        b'>' if depth <= 0 => {
          self.at += 1;
          return Ok(());
        },
        _ => {},
      }
      self.at += 1;
    }
    Err(self.error("a declaration that does not end"))
  }

  fn spaces(&mut self) {
    while self.at < self.s.len() && self.s[self.at].is_ascii_whitespace() {
      self.at += 1;
    }
  }

  fn name(&mut self) -> Result<String, XmlError> {
    let start = self.at;
    while self.at < self.s.len() {
      let c = self.s[self.at];
      if c.is_ascii_whitespace() || matches!(c, b'=' | b'>' | b'/' | b'<') {
        break;
      }
      self.at += 1;
    }
    if self.at == start {
      return Err(self.error("a missing name"));
    }
    let full = String::from_utf8_lossy(&self.s[start..self.at]);
    // Drop a namespace prefix, as in `svg:path` or `xlink:href`.
    Ok(match full.rsplit_once(':') {
      Some((_, local)) => local.to_string(),
      None => full.into_owned(),
    })
  }

  fn element(&mut self) -> Result<Node, XmlError> {
    self.at += 1; // '<'
    let name = self.name()?;
    let mut node = Node { name, ..Node::default() };
    loop {
      self.spaces();
      match self.s.get(self.at) {
        None => return Err(self.error("a tag that does not end")),
        Some(b'/') => {
          if self.s.get(self.at + 1) != Some(&b'>') {
            return Err(self.error("a stray / in a tag"));
          }
          self.at += 2;
          return Ok(node);
        },
        Some(b'>') => {
          self.at += 1;
          break;
        },
        Some(_) => {
          let attr = self.name()?;
          self.spaces();
          if self.s.get(self.at) != Some(&b'=') {
            return Err(self.error("an attribute without a value"));
          }
          self.at += 1;
          self.spaces();
          let quote = *self
            .s
            .get(self.at)
            .ok_or(self.error("a missing value"))?;
          if quote != b'"' && quote != b'\'' {
            return Err(self.error("an attribute value without quotes"));
          }
          self.at += 1;
          let start = self.at;
          while self.at < self.s.len() && self.s[self.at] != quote {
            self.at += 1;
          }
          if self.at >= self.s.len() {
            return Err(self.error("a value that does not end"));
          }
          let raw = String::from_utf8_lossy(&self.s[start..self.at]);
          self.at += 1;
          node.attributes.push((attr, unescape(&raw)));
        },
      }
    }
    // Children, until the end tag.
    loop {
      self.skip_misc()?;
      if self.at >= self.s.len() {
        return Err(self.error("an element that does not end"));
      }
      if self.starts("</") {
        self.at += 2;
        let end = self.name()?;
        if end != node.name {
          return Err(self.error("an end tag that does not match"));
        }
        self.spaces();
        if self.s.get(self.at) != Some(&b'>') {
          return Err(self.error("a bad end tag"));
        }
        self.at += 1;
        return Ok(node);
      }
      node.children.push(self.element()?);
    }
  }
}

/// Replaces the five named entities and character references.
fn unescape(s: &str) -> String {
  if !s.contains('&') {
    return s.to_string();
  }
  let mut out = String::with_capacity(s.len());
  let mut rest = s;
  while let Some(i) = rest.find('&') {
    out.push_str(&rest[..i]);
    rest = &rest[i..];
    let Some(end) = rest.find(';') else {
      break;
    };
    let entity = &rest[1..end];
    let c = match entity {
      "amp" => Some('&'),
      "lt" => Some('<'),
      "gt" => Some('>'),
      "quot" => Some('"'),
      "apos" => Some('\''),
      _ => entity
        .strip_prefix("#x")
        .and_then(|h| u32::from_str_radix(h, 16).ok())
        .or_else(|| {
          entity
            .strip_prefix('#')
            .and_then(|d| d.parse().ok())
        })
        .and_then(char::from_u32),
    };
    match c {
      Some(c) => {
        out.push(c);
        rest = &rest[end + 1..];
      },
      None => {
        out.push('&');
        rest = &rest[1..];
      },
    }
  }
  out.push_str(rest);
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn elements_attributes_and_skipped_parts() {
    let doc = r#"<?xml version="1.0"?>
      <!DOCTYPE svg [ <!ENTITY x "y"> ]>
      <!-- a comment -->
      <svg:svg xmlns:svg="http://www.w3.org/2000/svg" viewBox='0 0 24 24'>
        <g fill="none"><path d="M0 0L1 1" title="a &amp; b"/></g>
        text <![CDATA[ <ignored/> ]]>
        <circle r="2"></circle>
      </svg:svg>"#;
    let root = parse(doc).unwrap();
    assert_eq!(root.name, "svg");
    assert_eq!(root.get("viewBox"), Some("0 0 24 24"));
    assert_eq!(root.children.len(), 2);
    let path = &root.children[0].children[0];
    assert_eq!(path.get("d"), Some("M0 0L1 1"));
    assert_eq!(path.get("title"), Some("a & b"));
    assert_eq!(root.children[1].get("r"), Some("2"));
  }

  #[test]
  fn broken_documents_are_errors() {
    assert!(parse("<svg>").is_err());
    assert!(parse("<svg></g>").is_err());
    assert!(parse("<svg a=1/>").is_err());
    assert!(parse("").is_err());
  }

  #[test]
  fn character_references() {
    assert_eq!(unescape("&#65;&#x42;&bogus;"), "AB&bogus;");
  }
}
