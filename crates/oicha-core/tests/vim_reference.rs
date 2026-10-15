//! Explicitly-run differential tests against an installed, unconfigured Vim.
//! Vim is a test oracle only; the shipped application has no Vim dependency.
use oicha_core::{document::RichText, editor::Editor};
use std::{path::PathBuf, process::Command};

#[test]
#[ignore = "requires an installed Vim; run with --ignored"]
fn supported_commands_match_vim_reference() {
    let vim = std::env::var_os("OICHA_VIM_REFERENCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(windows) {
                PathBuf::from("C:/Program Files/Git/usr/bin/vim.exe")
            } else {
                PathBuf::from("vim")
            }
        });
    let cases: &[(&str, &str, usize, &[&str])] = &[
        ("zz keeps column", "abc\ndef\nghi", 5, &["z", "z"]),
        (
            "count zz keeps column",
            "abc\ndef\nghi",
            2,
            &["2", "z", "z"],
        ),
        (
            "count zz short line",
            "abcd\nx\nqrst",
            3,
            &["2", "z", "z", "3", "z", "z"],
        ),
        ("count zz beyond end", "abc\ndef", 1, &["9", "9", "z", "z"]),
        (
            "zz visual selection",
            "abcdef",
            2,
            &["v", "l", "z", "z", "d"],
        ),
        ("zz escape prefix", "abc", 1, &["z", "escape", "l"]),
        ("zz invalid suffix", "abc", 1, &["z", "h"]),
        ("h stops at beginning", "abc\ndef", 4, &["h"]),
        ("l stops at last character", "abc\ndef", 2, &["l"]),
        ("counted l stops", "abc\ndef", 0, &["9", "l"]),
        ("dollar", "abc\ndef", 0, &["$"]),
        ("counted dollar", "abc\ndef\nghi", 0, &["2", "$"]),
        ("G first nonblank", "abc\n  def", 0, &["G"]),
        ("count G", "abc\n  def\nghi", 0, &["2", "G"]),
        ("count gg", "abc\n  def\nghi", 8, &["2", "g", "g"]),
        ("x capped at line", "abc\ndef", 1, &["9", "x"]),
        ("X capped at line", "abc\ndef", 5, &["9", "X"]),
        ("dl at end", "abc\ndef", 2, &["d", "l"]),
        ("dh at beginning", "abc\ndef", 4, &["d", "h"]),
        ("dw", "one two three", 0, &["d", "w"]),
        ("dw at line end", "one two\nthree", 4, &["d", "w"]),
        ("2dw", "one two three four", 0, &["2", "d", "w"]),
        ("d2w", "one two three four", 0, &["d", "2", "w"]),
        ("cw", "one two", 0, &["c", "w", "insert:X", "escape"]),
        ("cw one letter", "a b", 0, &["c", "w", "insert:X", "escape"]),
        (
            "c2w",
            "one two three",
            0,
            &["c", "2", "w", "insert:X", "escape"],
        ),
        (
            "ciw end",
            "hello world",
            6,
            &["c", "i", "w", "insert:X", "escape"],
        ),
        ("yiw retains column", "hello world", 8, &["y", "i", "w"]),
        ("yy retains column", "hello\nworld", 2, &["y", "y"]),
        ("dd middle", "abc\ndef\nghi", 4, &["d", "d"]),
        ("dd last", "abc\ndef", 4, &["d", "d"]),
        (
            "cc middle",
            "abc\ndef\nghi",
            4,
            &["c", "c", "insert:X", "escape"],
        ),
        ("cc last", "abc\ndef", 4, &["c", "c", "insert:X", "escape"]),
        ("dd p", "abc\ndef", 0, &["d", "d", "p"]),
        ("yy p", "abc\ndef", 0, &["y", "y", "p"]),
        ("yy P", "abc\ndef", 4, &["y", "y", "P"]),
        ("character paste", "abcd", 0, &["v", "l", "y", "p"]),
        (
            "visual delete backwards",
            "abcdef",
            3,
            &["v", "h", "h", "d"],
        ),
        ("dgg", "abc\ndef\nghi", 4, &["d", "g", "g"]),
        ("dG", "abc\ndef\nghi", 4, &["d", "G"]),
        (
            "a empty line",
            "abc\n\ndef",
            4,
            &["a", "insert:X", "escape"],
        ),
        ("a end of line", "abc\ndef", 2, &["a", "insert:X", "escape"]),
        ("D", "abc\ndef", 1, &["D"]),
        ("undo", "abc\ndef", 1, &["x", "u"]),
        ("redo", "abc\ndef", 1, &["x", "u", "ctrl-r"]),
        ("join", "one\n  two", 0, &["J"]),
        ("Unicode x", "mùa gió", 0, &["x"]),
        ("word end", "one two", 0, &["e"]),
        ("word backward", "one two", 5, &["b"]),
        ("diw whitespace", "one   two", 4, &["d", "i", "w"]),
        ("daw end of line", "one two", 4, &["d", "a", "w"]),
        (
            "counted text object",
            "one two three",
            0,
            &["d", "2", "i", "w"],
        ),
        (
            "counted around word",
            "one two three",
            0,
            &["d", "2", "a", "w"],
        ),
        (
            "counted whitespace object",
            "one   two three",
            4,
            &["d", "2", "i", "w"],
        ),
        ("word across blank line", "one\n\ntwo", 0, &["w"]),
        ("word from blank line", "one\n\ntwo", 4, &["w"]),
        ("word backward to blank line", "one\n\ntwo", 5, &["b"]),
        ("word backward from blank line", "one\n\ntwo", 4, &["b"]),
        ("dk at first line", "one\ntwo", 0, &["d", "k"]),
        ("dj at last line", "one\ntwo", 4, &["d", "j"]),
        (
            "counted cc",
            "one\ntwo\nthree",
            0,
            &["2", "c", "c", "insert:X", "escape"],
        ),
        ("counted D", "one\ntwo\nthree", 1, &["2", "D"]),
        (
            "counted C",
            "one\ntwo\nthree",
            1,
            &["2", "C", "insert:X", "escape"],
        ),
        ("counted paste", "abc\ndef", 0, &["y", "y", "2", "p"]),
        (
            "linewise paste blank last line",
            "abc\n",
            4,
            &["y", "y", "p"],
        ),
        ("counted join", "one\ntwo\nthree", 0, &["3", "J"]),
        (
            "visual switches preserve anchor",
            "one\ntwo\nthree",
            0,
            &["v", "j", "V", "v", "d"],
        ),
    ];
    let directory = tempfile::tempdir().unwrap();
    let mut failures = vec![];
    for (name, text, start, keys) in cases {
        let output = directory.path().join("result.json");
        let script = directory.path().join("reference.vim");
        let line = text[..*start].bytes().filter(|b| *b == b'\n').count() + 1;
        let column = start - text[..*start].rfind('\n').map(|i| i + 1).unwrap_or(0) + 1;
        let mut sequence = String::new();
        let mut engine = Editor::new(RichText::plain(*text));
        engine.move_to(*start, false);
        for key in *keys {
            if let Some(insert) = key.strip_prefix("insert:") {
                engine.insert(insert);
                sequence.push_str(insert);
            } else {
                engine.key(key);
                sequence.push_str(match *key {
                    "escape" => "\\<Esc>",
                    "ctrl-r" => "\\<C-R>",
                    _ => key,
                });
            }
        }
        let script_text = format!(
            "set nocompatible\nset encoding=utf-8\nset virtualedit=\nset whichwrap=\ncall setline(1, {})\ncall cursor({line}, {column})\nlet &undolevels = &undolevels\nexecute \"normal! {sequence}\"\ncall writefile([json_encode({{'text': join(getline(1, '$'), \"\\n\"), 'line': line('.'), 'column': col('.') }})], '{}')\nqa!\n",
            serde_json::to_string(&text.split('\n').collect::<Vec<_>>()).unwrap(),
            output
                .to_string_lossy()
                .replace('\\', "/")
                .replace('\'', "''")
        );
        std::fs::write(&script, script_text).unwrap();
        let execution = Command::new(&vim)
            .args(["-Nu", "NONE", "-n", "-es", "-i", "NONE", "-V1", "-S"])
            .arg(script.to_string_lossy().replace('\\', "/"))
            .output()
            .expect("install Vim or set OICHA_VIM_REFERENCE");
        assert!(
            execution.status.success(),
            "reference Vim failed for {name}: {}",
            String::from_utf8_lossy(&execution.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap();
        let reference = result["text"].as_str().unwrap();
        let row = result["line"].as_u64().unwrap() as usize - 1;
        let cursor = reference
            .split('\n')
            .take(row)
            .map(|line| line.len() + 1)
            .sum::<usize>()
            + result["column"].as_u64().unwrap() as usize
            - 1;
        if engine.body.text != reference || engine.cursor != cursor {
            failures.push(format!(
                "{name}: Oicha {:?} @ {}; Vim {:?} @ {}",
                engine.body.text, engine.cursor, reference, cursor
            ));
        }
        engine.body.validate().unwrap();
    }
    assert!(
        failures.is_empty(),
        "{} discrepancies:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
