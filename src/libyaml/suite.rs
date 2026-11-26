use super::parser::{Event, Parser, ScalarStyle};
use std::borrow::Cow;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

fn esc(out: &mut String, b: &[u8]) {
    for c in String::from_utf8_lossy(b).chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\0' => out.push_str("\\0"),
            '\x08' => out.push_str("\\b"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
}

fn props(out: &mut String, anchor: Option<&[u8]>, tag: Option<&[u8]>) {
    if let Some(a) = anchor {
        out.push_str(" &");
        esc(out, a);
    }
    if let Some(t) = tag {
        out.push_str(" <");
        esc(out, t);
        out.push('>');
    }
}

fn events(input: &[u8]) -> Result<String, String> {
    let mut p = Parser::new(Cow::Borrowed(input));
    let mut out = String::new();
    loop {
        let (e, _) = p.next().map_err(|e| e.to_string())?;
        match e {
            Event::StreamStart => out.push_str("+STR"),
            Event::StreamEnd => {
                out.push_str("-STR\n");
                return Ok(out);
            }
            Event::DocumentStart => out.push_str("+DOC"),
            Event::DocumentEnd => out.push_str("-DOC"),
            Event::Alias(a) => {
                out.push_str("=ALI *");
                esc(&mut out, &a.0);
            }
            Event::Scalar(s) => {
                out.push_str("=VAL");
                props(&mut out, s.anchor.as_ref().map(|a| &*a.0), s.tag.as_deref());
                out.push(' ');
                out.push(match s.style {
                    ScalarStyle::Plain => ':',
                    ScalarStyle::SingleQuoted => '\'',
                    ScalarStyle::DoubleQuoted => '"',
                    ScalarStyle::Literal => '|',
                    ScalarStyle::Folded => '>',
                });
                esc(&mut out, &s.value);
            }
            Event::SequenceStart(s) => {
                out.push_str("+SEQ");
                props(&mut out, s.anchor.as_ref().map(|a| &*a.0), s.tag.as_deref());
            }
            Event::SequenceEnd => out.push_str("-SEQ"),
            Event::MappingStart(m) => {
                out.push_str("+MAP");
                props(&mut out, m.anchor.as_ref().map(|a| &*a.0), m.tag.as_deref());
            }
            Event::MappingEnd => out.push_str("-MAP"),
        }
        out.push('\n');
    }
}

fn norm(expected: &str) -> String {
    let mut out = String::new();
    for line in expected.lines() {
        let line = match line {
            "+DOC ---" => "+DOC",
            "-DOC ..." => "-DOC",
            _ => line,
        };
        let line = line.replacen("+MAP {}", "+MAP", 1);
        let line = line.replacen("+SEQ []", "+SEQ", 1);
        writeln!(out, "{line}").unwrap();
    }
    out
}

fn cases(dir: &Path, out: &mut Vec<PathBuf>) {
    if dir.join("in.yaml").exists() {
        out.push(dir.to_owned());
        return;
    }
    let mut sub: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    sub.sort();
    for d in sub {
        if d.is_dir() && !d.ends_with("name") && !d.ends_with("tags") {
            cases(&d, out);
        }
    }
}

#[test]
fn suite() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/suite");
    if !root.join("229Q").exists() {
        eprintln!("suite: run git submodule update --init");
        return;
    }
    let skip = fs::read_to_string(root.join("../skip")).unwrap();
    let skip: Vec<&str> = skip.lines().map(|l| l.split(':').next().unwrap()).collect();
    let mut all = Vec::new();
    cases(&root, &mut all);
    let (mut pass, mut fail) = (0, Vec::new());
    for dir in &all {
        let id = dir
            .strip_prefix(&root)
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let input = fs::read(dir.join("in.yaml")).unwrap();
        let ok = match events(&input) {
            Ok(got) => {
                !dir.join("error").exists()
                    && got == norm(&fs::read_to_string(dir.join("test.event")).unwrap())
            }
            Err(_) => dir.join("error").exists(),
        };
        if ok {
            pass += 1;
        } else if !skip.contains(&id.as_str()) {
            fail.push(id);
        }
    }
    eprintln!("suite: {pass} of {} pass", all.len());
    assert!(fail.is_empty(), "suite: unexpected failures: {fail:?}");
}
