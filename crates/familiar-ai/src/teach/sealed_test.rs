//! A planned test file as the judge sees it: its test functions at the seal commit,
//! never a bare diff, since a modified file's diff can hold no test function at all.
//! A new file is shown whole; a changed Python file shows every top-level test, the
//! changed ones first so the caller's cut keeps them; any other file is shown whole.

/// The line before the test functions the seal did not change: the caller's cut keeps what is
/// above it whole and applies to what is below.
pub const THE_REST: &str = "(the test functions below are ones the seal did not change)";

/// One top-level test function, or one part of a test class, with its first and last lines
/// (0-based, inclusive). A class is the lines of its own, `Class`, and each of its test methods,
/// `Class.test_x`, with its decorators and the helpers written after it.
#[derive(Debug, Clone, PartialEq)]
struct Block {
    name: String,
    /// Its first line, its first decorator when it has any.
    first: usize,
    /// The line of its `def` or `class`.
    at: usize,
    last: usize,
    /// The `class` line of the class a part belongs to; `None` for a top-level function.
    head: Option<usize>,
}

/// The planned test file `path` as the judge is shown it, given the file at the seal's parent
/// (`None` when new in the seal) and at the seal.
pub fn shown(path: &str, before: Option<&[u8]>, after: &[u8]) -> String {
    let Some(before) = before else {
        return format!("(new in the seal commit)\n{}", String::from_utf8_lossy(after));
    };
    let lines: Vec<String> = familiar_ai_git::split_lines(after).iter().map(|l| String::from_utf8_lossy(l).trim_end_matches(['\n', '\r']).to_string()).collect();
    let changed = changed_lines(before, after);
    let whole = |why: &str| format!("({why}: the whole file as at the seal)\nchanged in the seal: {}\n{}", ranges(&changed), String::from_utf8_lossy(after));
    if !path.ends_with(".py") {
        return whole("changed by the seal commit");
    }
    let blocks = python_blocks(&lines);
    if blocks.is_empty() {
        return whole("changed by the seal commit; no test function in it");
    }
    let touched = |b: &Block| changed.iter().any(|(a, z)| *a <= b.last && *z >= b.first);
    let (first, rest): (Vec<usize>, Vec<usize>) = (0..blocks.len()).partition(|&k| blocks.get(k).is_some_and(touched));
    let note = if first.is_empty() {
        format!("no test function (only {}, outside every test function)", ranges(&changed))
    } else {
        first.iter().filter_map(|&k| blocks.get(k)).map(|b| b.name.as_str()).collect::<Vec<_>>().join(", ")
    };
    let mut body = import_header(&lines).map(|h| format!("{h}\n\n")).unwrap_or_default();
    let mut prev: Option<usize> = None;
    for &k in first.iter().chain(rest.iter()) {
        let Some(b) = blocks.get(k) else { continue };
        if !first.is_empty() && Some(&k) == rest.first() {
            body.push_str(&format!("\n\n{THE_REST}\n\n"));
            prev = None;
        }
        // The next part of the same class, in file order, goes on as the file has it.
        let next = prev.and_then(|p| blocks.get(p)).filter(|p| p.head.is_some() && p.head == b.head && prev.map(|q| q + 1) == Some(k));
        match next {
            _ if body.ends_with("\n\n") => {}
            Some(p) => body.push_str(if p.last + 1 == b.first { "\n" } else { "\n\n" }),
            None if body.is_empty() => {}
            None => body.push_str("\n\n"),
        }
        // A method shown away from the lines before it says which class it is in.
        if let Some(h) = b.head.filter(|&h| h != b.at && next.is_none()) {
            body.push_str(lines.get(h).map_or("", String::as_str));
            body.push('\n');
        }
        body.push_str(&lines.get(b.first..=b.last).unwrap_or(&[]).join("\n"));
        prev = Some(k);
    }
    format!("(changed by the seal commit: its test functions as at the seal, the ones the seal changed first)\nchanged in the seal: {note}\n{body}\n")
}

/// Every test definition of a Python file as `python_blocks` reads it: its name and its first
/// and last lines, 0-based and inclusive, decorators included.
pub(crate) fn test_blocks(bytes: &[u8]) -> Vec<(String, usize, usize)> {
    let lines: Vec<String> = familiar_ai_git::split_lines(bytes).iter().map(|l| String::from_utf8_lossy(l).trim_end_matches(['\n', '\r']).to_string()).collect();
    python_blocks(&lines).into_iter().map(|b| (b.name, b.first, b.last)).collect()
}

/// The leading `import` and `from` lines of a Python file, up to the last one before its first
/// other statement, or `None` when it has none.
fn import_header(lines: &[String]) -> Option<String> {
    let mut last = None;
    let mut depth = 0;
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if depth > 0 {
            depth += bracket_depth(line);
            last = Some(i);
        } else if t.starts_with("import ") || t.starts_with("from ") {
            depth = bracket_depth(line).max(0);
            last = Some(i);
        } else if !(t.is_empty() || t.starts_with('#')) {
            break;
        }
    }
    last.map(|l| lines.get(..=l).unwrap_or(&[]).join("\n"))
}

/// The seal's changed lines on the new side, 0-based and inclusive.
/// A run of removed lines counts as touching the lines on either side of where it was.
pub(crate) fn changed_lines(before: &[u8], after: &[u8]) -> Vec<(usize, usize)> {
    let at = |n: u64| usize::try_from(n).unwrap_or(usize::MAX);
    familiar_ai_git::line_hunks(before, after)
        .into_iter()
        .map(|h| {
            let start = at(h.new_start).saturating_sub(1);
            match at(h.new_count) {
                0 => (start.saturating_sub(1), start),
                n => (start, start + n - 1),
            }
        })
        .collect()
}

/// The ranges as 1-based text such as `lines 3–4, 9`, or `no lines`.
fn ranges(changed: &[(usize, usize)]) -> String {
    if changed.is_empty() {
        return "no lines".to_string();
    }
    let one = |(a, z): &(usize, usize)| if a == z { format!("{}", a + 1) } else { format!("{}–{}", a + 1, z + 1) };
    let word = if matches!(changed, [(a, z)] if a == z) { "line" } else { "lines" };
    format!("{word} {}", changed.iter().map(one).collect::<Vec<_>>().join(", "))
}

/// The test name the top-level line `lines[i]` opens: a `def test_*`, an `async def test_*`, a
/// `class Test*`, a class whose bases name `TestCase`, or a class whose body holds a `def test_`.
fn opens(lines: &[String], i: usize) -> Option<String> {
    let line = lines.get(i)?;
    let rest = line.strip_prefix("async ").unwrap_or(line);
    let named = |n: &str| n.split(|ch: char| !(ch.is_alphanumeric() || ch == '_')).next().unwrap_or("").to_string();
    if let Some(n) = rest.strip_prefix("def ").filter(|n| n.starts_with("test_")) {
        return Some(named(n));
    }
    let n = rest.strip_prefix("class ")?;
    (n.starts_with("Test") || line.contains("TestCase") || holds_a_test_method(lines, i)).then(|| named(n))
}

/// Whether the class opened at `lines[i]` has a `def test_` method in its body.
fn holds_a_test_method(lines: &[String], i: usize) -> bool {
    let body = lines.iter().skip(i + 1).take_while(|l| l.is_empty() || l.starts_with([' ', '\t', '#', ')']));
    body.map(|l| l.trim_start()).any(|t| t.strip_prefix("async ").unwrap_or(t).starts_with("def test_"))
}

/// Every top-level test function of a Python file, and every test class as its own lines and
/// each of its test methods, in file order, each from its decorators to its last non-blank,
/// non-comment line before the next statement at its level.
fn python_blocks(lines: &[String]) -> Vec<Block> {
    let in_str = in_strings(lines);
    scan(lines, &in_str, 0, lines.len(), 0, false, &|i| opens(lines, i)).into_iter().flat_map(|b| split_class(lines, &in_str, b)).collect()
}

/// The lines that begin inside a triple-quoted string.
fn in_strings(lines: &[String]) -> Vec<bool> {
    let mut quote: Option<&str> = None;
    lines
        .iter()
        .map(|line| {
            let was = quote.is_some();
            quote = toggled(quote, line);
            was
        })
        .collect()
}

/// A test class cut into its own lines and its test methods; any other block as it is.
fn split_class(lines: &[String], in_str: &[bool], b: Block) -> Vec<Block> {
    let is_class = lines.get(b.at).is_some_and(|l| l.starts_with("class "));
    // The methods stand at the indent of the first line of the body.
    let indent = (b.at + 1..=b.last).find(|&i| !in_str.get(i).copied().unwrap_or(true) && lines.get(i).is_some_and(|l| !l.trim().is_empty() && !l.trim_start().starts_with(['#', ')'])));
    let Some(indent) = indent.filter(|_| is_class).and_then(|i| lines.get(i)).map(|l| l.len() - l.trim_start().len()) else {
        return vec![b];
    };
    let method = |i: usize| {
        let t = lines.get(i)?.trim_start();
        let name = t.strip_prefix("async ").unwrap_or(t).strip_prefix("def ").filter(|n| n.starts_with("test_"))?;
        Some(name.split(|ch: char| !(ch.is_alphanumeric() || ch == '_')).next().unwrap_or("").to_string())
    };
    let methods = scan(lines, in_str, b.at + 1, b.last + 1, indent, true, &method);
    let Some(m) = methods.first() else { return vec![b] };
    let own_last = (b.first..m.first).rev().find(|&i| lines.get(i).is_some_and(|l| !l.trim().is_empty())).unwrap_or(b.at);
    let head = Some(b.at);
    let mut out = vec![Block { last: own_last, head, ..b.clone() }];
    out.extend(methods.into_iter().map(|m| Block { name: format!("{}.{}", b.name, m.name), head, ..m }));
    out
}

/// The statements at column `indent` among `lines[lo..hi]` that `opens` names, each with its
/// decorators. A statement `opens` does not name ends the block before it, unless `attach`, which
/// keeps it (a helper written between two test methods) as part of that block.
fn scan(lines: &[String], in_str: &[bool], lo: usize, hi: usize, indent: usize, attach: bool, opens: &dyn Fn(usize) -> Option<String>) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut open: Option<Block> = None;
    // The first line of the decorators waiting for their `def`, and how many of their brackets are open.
    let mut deco: Option<usize> = None;
    let mut depth = 0;
    for i in lo..hi.min(lines.len()) {
        let line = lines.get(i).map_or("", String::as_str);
        let t = line.trim();
        if in_str.get(i).copied().unwrap_or(false) {
            if let Some(b) = open.as_mut() {
                b.last = i;
            }
            continue;
        }
        if t.is_empty() {
            continue;
        }
        if depth > 0 {
            depth += bracket_depth(line);
            continue;
        }
        let at = line.len() - line.trim_start().len();
        let comment = t.starts_with('#');
        if at != indent || comment || t.starts_with([')', ']', '}']) {
            // A comment at the block's own column belongs to no block, so it never extends one.
            if let Some(b) = open.as_mut().filter(|_| !(comment && at <= indent)) {
                b.last = i;
            }
            continue;
        }
        if t.starts_with('@') {
            deco.get_or_insert(i);
            depth = bracket_depth(line).max(0);
            continue;
        }
        let first = deco.take().unwrap_or(i);
        match opens(i) {
            Some(name) => {
                out.extend(open.replace(Block { name, first, at: i, last: i, head: None }));
            }
            None if attach => {
                if let Some(b) = open.as_mut() {
                    b.last = i;
                }
            }
            None => out.extend(open.take()),
        }
    }
    out.extend(open);
    out
}

/// How many brackets `line` opens beyond those it closes, not counting those in a quoted string or comment.
fn bracket_depth(line: &str) -> i32 {
    let mut depth = 0;
    let mut quote: Option<char> = None;
    let mut chars = line.chars();
    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (Some(_), '\\') => {
                chars.next();
            }
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '#') => break,
            (None, '\'' | '"') => quote = Some(ch),
            (None, '(' | '[' | '{') => depth += 1,
            (None, ')' | ']' | '}') => depth -= 1,
            _ => {}
        }
    }
    depth
}

/// The triple-quoted string open after `line`, given the one open before it.
fn toggled<'a>(quote: Option<&'a str>, line: &str) -> Option<&'a str> {
    match quote {
        Some(q) => (line.matches(q).count() % 2 == 0).then_some(q),
        None => ["\"\"\"", "'''"].into_iter().find(|q| line.matches(q).count() % 2 == 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT_SEAL: &str = "\
import pytest
from pydantic import BaseModel, Field


def helper():
    return 1


@pytest.mark.parametrize('x', [1, 2])
def test_alias(
    x,
):
    s = '''
not a top-level line
'''
    assert x


# a comment between tests
class TestModel:
    def test_one(self):
        assert True

async def test_last():
    assert helper() == 1
";

    #[test]
    fn a_file_new_at_the_seal_is_shown_whole() {
        assert_eq!(shown("tests/test_a.py", None, b"a\nb\n"), "(new in the seal commit)\na\nb\n");
    }

    #[test]
    fn the_blocks_keep_decorators_bodies_and_strings_and_stop_at_the_next_statement() {
        let lines: Vec<String> = AT_SEAL.lines().map(str::to_string).collect();
        let b = python_blocks(&lines);
        let named: Vec<(&str, usize, usize)> = b.iter().map(|b| (b.name.as_str(), b.first + 1, b.last + 1)).collect();
        assert_eq!(named, [("test_alias", 9, 16), ("TestModel", 20, 20), ("TestModel.test_one", 21, 22), ("test_last", 24, 25)]);
    }

    #[test]
    fn a_seal_that_changed_only_an_import_shows_every_test_function_and_says_so() {
        // The seal's diff of the file is one import line.
        let before = AT_SEAL.replace("BaseModel, Field", "BaseModel");
        let s = shown("tests/test_a.py", Some(before.as_bytes()), AT_SEAL.as_bytes());
        assert!(s.starts_with("(changed by the seal commit: its test functions as at the seal, the ones the seal changed first)\nchanged in the seal: no test function (only line 2, outside every test function)\nimport pytest\nfrom pydantic import BaseModel, Field\n\n@pytest.mark.parametrize('x', [1, 2])\ndef test_alias(\n"), "{s}");
        assert!(s.contains("    assert x\n\nclass TestModel:\n    def test_one(self):\n        assert True\n\nasync def test_last():\n"), "{s}");
        assert!(!s.contains("def helper") && !s.contains("# a comment"), "{s}");
    }

    #[test]
    fn the_functions_the_seal_changed_come_first() {
        let before = AT_SEAL.replace("        assert True\n", "        assert 1\n").replace("    assert helper() == 1\n", "    assert helper() == 2\n");
        let s = shown("tests/test_a.py", Some(before.as_bytes()), AT_SEAL.as_bytes());
        assert!(s.contains("changed in the seal: TestModel.test_one, test_last\nimport pytest\nfrom pydantic import BaseModel, Field\n\nclass TestModel:\n    def test_one(self):"), "{s}");
        assert!(s.find("async def test_last").unwrap_or(0) < s.find("def test_alias").unwrap_or(0), "{s}");
        // a run of removed lines touches the function it was cut from
        let before = AT_SEAL.replace("    assert helper() == 1\n", "    assert helper() == 1\n    assert helper() > 0\n");
        assert!(shown("tests/test_a.py", Some(before.as_bytes()), AT_SEAL.as_bytes()).contains("changed in the seal: test_last\n"));
    }

    #[test]
    fn other_languages_and_python_without_a_test_function_are_shown_whole() {
        let s = shown("tests/test_a.sh", Some(b"a\nb\n"), b"a\nc\nd\n");
        assert_eq!(s, "(changed by the seal commit: the whole file as at the seal)\nchanged in the seal: lines 2–3\na\nc\nd\n");
        let s = shown("tests/conftest.py", Some(b"x = 1\n"), b"x = 2\n");
        assert_eq!(s, "(changed by the seal commit; no test function in it: the whole file as at the seal)\nchanged in the seal: line 1\nx = 2\n");
    }

    const UNITTEST: &str = "\
import unittest


class Helper:
    def go(self):
        return 1


class CheckThings(unittest.TestCase):
    def test_a(self):
        assert 1


class Holds:
    def test_b(self):
        assert 2


class Bare(unittest.TestCase):
    pass
";

    #[test]
    fn a_class_holding_a_test_method_or_named_for_testcase_is_shown_and_a_plain_class_is_not() {
        let before = UNITTEST.replace("import unittest\n", "import unittest, os\n");
        let s = shown("tests/test_a.py", Some(before.as_bytes()), UNITTEST.as_bytes());
        assert!(s.contains("class CheckThings(unittest.TestCase):\n    def test_a(self):\n        assert 1\n"), "{s}");
        assert!(s.contains("class Holds:\n    def test_b(self):\n        assert 2\n"), "{s}");
        assert!(s.contains("class Bare(unittest.TestCase):\n    pass\n"), "{s}");
        assert!(!s.contains("class Helper") && !s.contains("def go"), "{s}");
    }

    #[test]
    fn a_decorator_over_several_lines_is_kept_with_its_def() {
        let src = "import pytest\n\n\n@pytest.mark.parametrize(\n    'x',\n    [1, 2],\n)\ndef test_many(x):\n    assert x\n\n\ndef test_two():\n    assert 2\n";
        let lines: Vec<String> = src.lines().map(str::to_string).collect();
        let blocks = python_blocks(&lines);
        let named: Vec<(&str, usize, usize)> = blocks.iter().map(|b| (b.name.as_str(), b.first + 1, b.last + 1)).collect();
        assert_eq!(named, [("test_many", 4, 9), ("test_two", 12, 13)]);
        let before = src.replace("assert 2", "assert 3");
        let s = shown("tests/test_a.py", Some(before.as_bytes()), src.as_bytes());
        assert!(s.contains("@pytest.mark.parametrize(\n    'x',\n    [1, 2],\n)\ndef test_many(x):\n    assert x\n"), "{s}");
    }

    const BIG_CLASS: &str = "\
import unittest


class CheckThings(unittest.TestCase):
    def setUp(self):
        self.n = 1

    def test_first(self):
        assert self.n

    @unittest.skip(
        'later',
    )
    def test_second(self):
        assert self.n == 1

    def helper(self):
        return 2

    def test_third(self):
        assert self.helper() == 2


def test_after():
    assert True
";

    #[test]
    fn a_changed_method_of_a_big_class_is_shown_before_the_unchanged_ones() {
        let before = BIG_CLASS.replace("assert self.helper() == 2", "assert self.helper() == 3");
        let s = shown("tests/test_a.py", Some(before.as_bytes()), BIG_CLASS.as_bytes());
        assert!(s.contains("changed in the seal: CheckThings.test_third\n"), "{s}");
        let at = |needle: &str| s.find(needle).unwrap_or(usize::MAX);
        assert!(at("def test_third") < at("def test_first") && at("def test_first") < at("def test_second") && at("def test_second") < at("def test_after"), "{s}");
        // a method taken out of its class order is still under its class line
        assert!(s.contains("class CheckThings(unittest.TestCase):\n    def test_third(self):\n        assert self.helper() == 2\n"), "{s}");
        // a helper between two methods stays with the method before it
        assert!(s.contains("        assert self.n == 1\n\n    def helper(self):\n        return 2\n"), "{s}");
        // a changed method's decorators go with it, over however many lines
        let before = BIG_CLASS.replace("assert self.n == 1", "assert self.n == 2");
        let s = shown("tests/test_a.py", Some(before.as_bytes()), BIG_CLASS.as_bytes());
        assert!(s.contains("class CheckThings(unittest.TestCase):\n    @unittest.skip(\n        'later',\n    )\n    def test_second(self):\n"), "{s}");
        // the class's own lines (setUp) are a block of their own, named for the class
        let before = BIG_CLASS.replace("self.n = 1", "self.n = 0");
        let s = shown("tests/test_a.py", Some(before.as_bytes()), BIG_CLASS.as_bytes());
        assert!(s.contains(&format!("changed in the seal: CheckThings\nimport unittest\n\nclass CheckThings(unittest.TestCase):\n    def setUp(self):\n        self.n = 1\n\n{THE_REST}\n\nclass CheckThings(unittest.TestCase):\n    def test_first(self):")), "{s}");
    }

    #[test]
    fn the_import_header_is_shown_before_the_blocks_so_a_missing_import_can_be_seen() {
        let src = "import os\nfrom a import (\n    b,\n    c,\n)\n\nimport sys\n\nVALUE = 1\n\n\ndef test_uses(shout):\n    assert shout(c) == os.sep\n";
        let before = src.replace("assert shout(c)", "assert shout(b)");
        let s = shown("tests/test_a.py", Some(before.as_bytes()), src.as_bytes());
        let header = "import os\nfrom a import (\n    b,\n    c,\n)\n\nimport sys\n";
        assert!(s.contains(&format!("changed in the seal: test_uses\n{header}\ndef test_uses(shout):\n")), "{s}");
        assert!(!s.contains("VALUE"), "{s}");
        // `shout` is used by the test and imported by no line of the header
        let shown_header = s.split("def test_uses").next().unwrap_or("");
        assert!(!shown_header.contains("shout"), "{s}");
        // a file with no import gets no header
        let bare = "def test_x():\n    assert 1\n";
        let s = shown("tests/test_a.py", Some(bare.replace("assert 1", "assert 2").as_bytes()), bare.as_bytes());
        assert!(s.contains("changed in the seal: test_x\ndef test_x"), "{s}");
    }
}
