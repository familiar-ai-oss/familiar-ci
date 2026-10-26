//! A small element tree over `quick-xml` (pure Rust), the one place the converters touch the
//! parser. It reads what a report file says and nothing else: names are local (a namespace
//! prefix is dropped), entities are the five predefined and character references (a DOCTYPE is
//! skipped, never expanded, so there is no external entity to fetch and no expansion to blow up),
//! and a document that is cut, empty, not XML, or nested deeper than [`MAX_DEPTH`] is an error:
//! a report that cannot be read whole is not evidence.

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

/// Reports nest a few levels; anything deeper is not a report (and a deep tree is a deep drop).
pub const MAX_DEPTH: usize = 256;

#[derive(Debug, Default, Clone)]
pub struct Element {
    /// The local name: `ns:testsuite` is `testsuite`.
    pub name: String,
    /// Attributes as written (a prefixed name stays prefixed), values unescaped.
    pub attrs: Vec<(String, String)>,
    /// The element's own text and CDATA, concatenated; its children's are in the children.
    pub text: String,
    pub children: Vec<Element>,
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// The first direct child with this name.
    pub fn child(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Every direct child with this name, in document order.
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// Every descendant (not this element) with this name, in document order.
    pub fn descendants<'a>(&'a self, name: &str) -> Vec<&'a Element> {
        let mut out = Vec::new();
        let mut stack: Vec<&Element> = self.children.iter().rev().collect();
        while let Some(e) = stack.pop() {
            if e.name == name {
                out.push(e);
            }
            stack.extend(e.children.iter().rev());
        }
        out
    }
}

fn element_of(e: &BytesStart<'_>) -> Result<Element, String> {
    let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
    let mut attrs = Vec::new();
    for a in e.attributes() {
        let a = a.map_err(|x| format!("an attribute of <{name}>: {x}"))?;
        let key = String::from_utf8_lossy(a.key.as_ref()).into_owned();
        let value = a.normalized_value(XmlVersion::Implicit1_0).map_err(|x| format!("the attribute {key} of <{name}>: {x}"))?.into_owned();
        attrs.push((key, value));
    }
    Ok(Element { name, attrs, text: String::new(), children: Vec::new() })
}

fn attach(stack: &mut [Element], root: &mut Option<Element>, done: Element) -> Result<(), String> {
    match stack.last_mut() {
        Some(parent) => parent.children.push(done),
        None if root.is_none() => *root = Some(done),
        None => return Err("more than one root element".into()),
    }
    Ok(())
}

/// The document's root element, or why it cannot be read whole.
pub fn parse(text: &str) -> Result<Element, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut reader = Reader::from_str(text);
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;
    loop {
        let event = reader.read_event().map_err(|e| format!("{e} (at byte {})", reader.buffer_position()))?;
        match event {
            Event::Start(e) => {
                if stack.len() >= MAX_DEPTH {
                    return Err(format!("nested deeper than {MAX_DEPTH} elements"));
                }
                if stack.is_empty() && root.is_some() {
                    return Err("more than one root element".into());
                }
                stack.push(element_of(&e)?);
            }
            Event::Empty(e) => {
                let done = element_of(&e)?;
                attach(&mut stack, &mut root, done)?;
            }
            Event::End(_) => {
                let done = stack.pop().ok_or("an end tag with no start")?;
                attach(&mut stack, &mut root, done)?;
            }
            Event::Text(t) => {
                let s = t.decode().map_err(|e| e.to_string())?;
                match stack.last_mut() {
                    Some(top) => top.text.push_str(&s.replace("\r\n", "\n")),
                    None if s.trim().is_empty() => {}
                    None => return Err("text outside the root element".into()),
                }
            }
            Event::CData(c) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&c.decode().map_err(|e| e.to_string())?.replace("\r\n", "\n"));
                }
            }
            Event::GeneralRef(r) => {
                if let Some(top) = stack.last_mut() {
                    if let Some(c) = r.resolve_char_ref().map_err(|e| e.to_string())? {
                        top.text.push(c);
                    } else {
                        let name = r.decode().map_err(|e| e.to_string())?;
                        match name.as_ref() {
                            "amp" => top.text.push('&'),
                            "lt" => top.text.push('<'),
                            "gt" => top.text.push('>'),
                            "quot" => top.text.push('"'),
                            "apos" => top.text.push('\''),
                            // declared in a DOCTYPE: never expanded, kept as written
                            other => {
                                top.text.push('&');
                                top.text.push_str(other);
                                top.text.push(';');
                            }
                        }
                    }
                }
            }
            Event::Eof => break,
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
    }
    if let Some(open) = stack.last() {
        return Err(format!("the document ends inside <{}>", open.name));
    }
    root.ok_or_else(|| "no element in the document".to_string())
}
