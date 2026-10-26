//! Lane AKreports (plan of record 2026-10-19, A section: "never-seen report files (truncated, ...)
//! read can't-tell"). A report file can be cut at any byte, by a killed run or a full disk: the
//! converters never raise, and a cut report is never read as something the whole is not.
//!
//! The sweep takes each recorded report (surefire, Gradle, PHPUnit's log, a TRX) and each hand-made
//! C/C++ sample and cuts it at every character boundary: the converter must not panic, and what
//! the reader makes of the cut is either "no structured output" (the next reader decides) or
//! exactly what it makes of the whole (a cut that took only trailing white space). The soak feeds
//! the converters thousands of scrambled documents: no panic, and no session unless a whole
//! report was read.

mod probe_runs_support;

use familiar_ai::door::convert::{junit, trx};
use probe_runs_support::*;

fn prefixes(text: &str) -> impl Iterator<Item = &str> {
    (0..=text.len()).filter(move |i| text.is_char_boundary(*i)).map(move |i| &text[..i])
}

fn sweep(name: &str, text: &str, convert: &dyn Fn(&str) -> String, exit: i32) -> usize {
    let whole = read_schema(&convert(text), exit);
    assert!(matches!(whole, Reading::Read { .. }), "{name}: the whole report reads: {whole:?}");
    let mut cuts = 0;
    for cut in prefixes(text) {
        let reading = read_schema(&convert(cut), exit);
        assert!(matches!(reading, Reading::NextReader(_)) || reading == whole, "{name}: cut at byte {} of {} reads {reading:?}, the whole reads {whole:?}", cut.len(), text.len());
        cuts += 1;
    }
    cuts
}

fn one(files: Vec<(String, String)>) -> String {
    assert_eq!(files.len(), 1, "one report file");
    files.into_iter().next().unwrap().1
}

#[test]
fn every_recorded_report_cut_at_every_byte_reads_unread_or_as_the_whole() {
    let mut cuts = 0;
    for probe in ["P2", "P7"] {
        let d = recording("maven", "plain", probe);
        let xml = one(files_under(&d.join("report/surefire-reports"), &|n| n.starts_with("TEST-") && n.ends_with(".xml")));
        cuts += sweep(&format!("maven {probe}"), &xml, &|t| junit::to_report(t, exit_of(&d)), exit_of(&d));
        let d = recording("gradle", "plain", probe);
        let xml = one(files_under(&d.join("report/test-results"), &|n| n.ends_with(".xml")));
        cuts += sweep(&format!("gradle {probe}"), &xml, &|t| junit::to_report(t, exit_of(&d)), exit_of(&d));
    }
    for probe in ["P2", "P3", "P5", "P7"] {
        let d = recording("phpunit", "plain-junit", probe);
        let xml = one(files_under(&d.join("report"), &|n| n == "junit.xml"));
        cuts += sweep(&format!("phpunit {probe}"), &xml, &|t| junit::to_report(t, exit_of(&d)), exit_of(&d));
    }
    for probe in ["P2", "P5", "P7"] {
        let d = recording("dotnet-xunit", "plain", probe);
        let doc = one(files_under(&d.join("report"), &|n| n.ends_with(".trx")));
        cuts += sweep(&format!("dotnet {probe}"), &doc, &|t| trx::to_report(t, exit_of(&d)), exit_of(&d));
    }
    assert!(cuts > 10_000, "the sweep cut {cuts} times");
}

#[test]
fn a_cut_c_and_cpp_sample_reads_unread_or_as_the_whole() {
    let gtest = "<testsuites tests=\"2\" failures=\"1\"><testsuite name=\"S\" tests=\"2\" failures=\"1\"><testcase name=\"A\" status=\"run\" result=\"completed\" classname=\"S\"/><testcase name=\"B\" status=\"run\" result=\"completed\" classname=\"S\"><failure message=\"b.cc:3&#x0A;boom\" type=\"\"><![CDATA[b.cc:3\nboom]]></failure></testcase></testsuite></testsuites>\n";
    let catch2 = "<testsuites><testsuite name=\"t\" errors=\"0\" failures=\"1\" tests=\"1\"><testcase classname=\"t.global\" name=\"a\" time=\"0.1\"><failure message=\"1 == 2\" type=\"REQUIRE\">\nFAILED:\n  REQUIRE( 1 == 2 )\nat /src/a.cpp:9\n</failure></testcase></testsuite></testsuites>\n";
    let ctest = "<testsuite name=\"(empty)\" tests=\"2\" failures=\"1\"><testcase name=\"a\" classname=\"a\" status=\"run\"/><testcase name=\"b\" classname=\"b\" status=\"fail\"><failure message=\"Failed\"/></testcase></testsuite>\n";
    for (name, doc) in [("gtest", gtest), ("catch2", catch2), ("ctest", ctest)] {
        sweep(name, doc, &|t| junit::to_report(t, 1), 1);
    }
}

/// A small deterministic generator: the soak must give the same documents on every run.
struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[test]
fn scrambled_documents_never_panic_and_never_read_without_a_whole_report() {
    const PIECES: [&str; 30] = [
        "<testsuite", "<testsuites>", "</testsuite>", "</testsuites>", "<testcase name=\"a\" classname=\"c\"", "/>", ">", "<", "&", "&amp;", "&#x0A;", "&#xD800;", "&bogus;", "<![CDATA[", "]]>", "<!--", "-->",
        "<failure message=\"m\" type=\"AssertionError\">", "</failure>", "<TestRun xmlns=\"x\">", "<UnitTestResult testId=\"a\" testName=\"T.a\" outcome=\"Passed\"/>", "<Results>", "</Results>", "\u{feff}", "\u{0}", "é", "\"", "'", "\n", " ",
    ];
    let mut rng = Xorshift(0x9E37_79B9_7F4A_7C15);
    let mut read = 0;
    for _ in 0..20_000 {
        let n = 1 + rng.below(40);
        let doc: String = (0..n).map(|_| PIECES[rng.below(PIECES.len())]).collect();
        for out in [junit::to_report(&doc, 1), trx::to_report(&doc, 1)] {
            // whatever came out is JSON Lines, and a session comes with its end: never one without the other
            let lines: Vec<serde_json::Value> = out.lines().map(|l| serde_json::from_str(l).expect("every line is JSON")).collect();
            let session = lines.iter().any(|l| l["type"] == "session");
            let end = lines.iter().any(|l| l["type"] == "end");
            assert_eq!(session, end, "a session and an end go together: {doc:?} gave {out}");
            if session {
                read += 1;
            }
        }
    }
    // documents that happened to be whole reports exist among the soak, but they are rare
    assert!(read < 2_000, "the soak read {read} of 40000 as whole reports");
}
