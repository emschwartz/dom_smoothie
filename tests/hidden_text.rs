//! Text that a human reader cannot see must never reach the extracted text, no matter how it
//! was hidden. The fixture tags every hidden payload with `HIDDEN_` and every payload that a
//! reader can see with `VISIBLE_`.

use std::fs;

use dom_smoothie::{Config, Readability, TextMode};

fn tokens(prefix: &str, html: &str) -> Vec<String> {
    let mut out = vec![];
    let mut rest = html;
    while let Some(pos) = rest.find(prefix) {
        let start = &rest[pos..];
        let end = start
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(start.len());
        let tok = start[..end].to_string();
        if !out.contains(&tok) {
            out.push(tok);
        }
        rest = &start[end..];
    }
    out
}

fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}' | '\u{200B}' | '\u{2060}' | '\u{FEFF}' | '\u{202A}'..='\u{202E}' | '\u{E0000}'..='\u{E007F}'
    )
}

#[test]
fn hidden_text_does_not_reach_text_content() {
    let html = fs::read_to_string("./test-pages/hidden-text/source.html").unwrap();
    let hidden = tokens("HIDDEN_", &html);
    let visible = tokens("VISIBLE_", &html);
    assert!(
        hidden.len() > 60 && visible.len() > 10,
        "fixture lost its payloads"
    );

    for mode in [TextMode::Raw, TextMode::Formatted] {
        let cfg = Config {
            text_mode: mode,
            ..Default::default()
        };
        let mut readability = Readability::new(html.as_str(), None, Some(cfg)).unwrap();
        let article = readability.parse().unwrap();
        let text = article.text_content.as_ref();

        let leaked: Vec<_> = hidden
            .iter()
            .filter(|t| text.contains(t.as_str()))
            .collect();
        assert!(
            leaked.is_empty(),
            "hidden text leaked ({mode:?}): {leaked:?}"
        );

        let lost: Vec<_> = visible
            .iter()
            .filter(|t| !text.contains(t.as_str()))
            .collect();
        assert!(lost.is_empty(), "visible text lost ({mode:?}): {lost:?}");

        assert!(
            text.contains("VISIBLE_zwsp_joined"),
            "zero-width spaces must be stripped from the text"
        );
        assert!(
            !text.chars().any(is_invisible),
            "invisible characters remain in the text"
        );
        assert!(article.content.contains("VISIBLE_xmp"));
    }
}
