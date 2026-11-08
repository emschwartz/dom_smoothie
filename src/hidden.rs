//! Detection of content that is present in the DOM but never shown to a human reader.
//!
//! Readability.js only looks at the `hidden` attribute, `aria-hidden="true"` and an inline
//! `display: none` / `visibility: hidden`. That is enough for ordinary pages, but it is trivial
//! to bypass on purpose, and text that a reader cannot see must not end up in the extracted
//! article (for example as instructions aimed at a language model that consumes the text).
//!
//! This module closes the cheap gaps without a real CSS engine:
//!
//! * `style_hiding` understands the inline-style tricks that make text invisible
//!   (zero opacity or font size, transparent color, off-screen positioning, zero-sized boxes
//!   with hidden overflow, `clip`, `clip-path`, `transform: scale(0)`, ...).
//! * [`apply_stylesheet_hiding`] scans `<style>` elements for simple class and id rules that
//!   hide their subjects and marks the matching elements with the `hidden` attribute, so that
//!   the regular visibility check removes them.
//! * [`is_probably_visible`] also knows the "visually hidden" class names of popular CSS
//!   frameworks, which are usually delivered by external stylesheets we never see.
//! * [`remove_unrendered_content`] drops fallback content of `<video>`, `<audio>`, `<canvas>`,
//!   `<meter>` and `<progress>`, closed `<dialog>` elements, `<datalist>`, ruby parentheses and
//!   the non-rendered metadata children of `<svg>` and `<math>`.
//! * [`strip_invisible_chars`] removes zero-width, bidi-control and Unicode "tag" characters
//!   that render as nothing but still reach a text consumer.
//!
//! Everything here is heuristic. It cannot see external stylesheets, computed styles,
//! same-colored text and background, or elements covered by other elements.

use dom_query::{Document, Matcher, Node, NodeData};
use foldhash::{HashMap, HashSet};
use phf::phf_set;
use std::rc::Rc;
use tendril::StrTendril;

/// Class tokens that conventionally map to a hiding rule in widespread CSS frameworks
/// (Bootstrap, Tailwind, Foundation, Bulma, WordPress, Drupal, SUIT).
#[rustfmt::skip]
static HIDDEN_CLASS_TOKENS: phf::Set<&'static str> = phf_set!(
    "hidden", "hide", "invisible", "d-none", "is-hidden", "u-hidden",
    "sr-only", "sr_only", "visually-hidden", "visuallyhidden", "visually_hidden",
    "visually-hidden-focusable", "screen-reader-text", "screen-reader-only",
    "screenreader-text", "a11y-hidden", "assistive-text", "element-invisible",
    "offscreen", "off-screen",
);

/// Returns `true` if the node is probably visible to a human reader.
///
/// Like Readability.js, every element is treated as invisible when it carries the `hidden`
/// attribute, `aria-hidden="true"` (unless it is a math fallback image) or an inline
/// `display: none` / `visibility: hidden`.
///
/// Elements that contain text are additionally checked for the other inline-style tricks
/// (see `style_hiding`) and for the "visually hidden" class names of popular CSS
/// frameworks. Text-free elements (images, placeholders, empty anchors) are left alone,
/// because lazy-loading patterns routinely keep them at zero opacity until a script runs.
pub(crate) fn is_probably_visible(node: &Node) -> bool {
    if node.has_attr("hidden") {
        return false;
    }

    match node.attr("style").and_then(|s| style_hiding(&s)) {
        Some(Hiding::Removed) => return false,
        Some(Hiding::Visual) if has_text(node) => return false,
        _ => {}
    }

    let class_attr = node.attr("class");

    let is_aria_hidden = node
        .attr("aria-hidden")
        .map_or(false, |a| a.trim().eq_ignore_ascii_case("true"));
    let has_fallback_image = class_attr
        .as_ref()
        .map_or(false, |s| s.contains("fallback-image"));

    if is_aria_hidden && !has_fallback_image {
        return false;
    }

    let has_hidden_class = class_attr.map_or(false, |c| {
        c.split_ascii_whitespace()
            .any(|token| HIDDEN_CLASS_TOKENS.contains(&token.to_ascii_lowercase()))
    });
    if has_hidden_class && has_text(node) {
        return false;
    }

    true
}

fn has_text(node: &Node) -> bool {
    let mut stack = vec![node.clone()];
    while let Some(current) = stack.pop() {
        let text_has_content = current
            .query(|tree_node| match tree_node.data {
                NodeData::Text { ref contents } => {
                    Some(contents.chars().any(|c| !c.is_whitespace()))
                }
                _ => None,
            })
            .flatten();
        match text_has_content {
            Some(true) => return true,
            Some(false) => {}
            None => stack.extend(current.children_it(true)),
        }
    }
    false
}

/// Collects the text nodes under `root` (not including `root` itself).
///
/// Deliberately not `descendants_it`: in `dom_query` 0.12 that iterator does not stop at
/// the subtree root and continues into the root's following siblings.
fn text_nodes<'a>(root: &Node<'a>) -> Vec<Node<'a>> {
    let mut out = vec![];
    let mut stack: Vec<Node<'a>> = root.children_it(true).collect();
    while let Some(current) = stack.pop() {
        let is_text = current
            .query(|tree_node| matches!(tree_node.data, NodeData::Text { .. }))
            .unwrap_or(false);
        if is_text {
            out.push(current);
        } else {
            stack.extend(current.children_it(true));
        }
    }
    out
}

/// How a declaration block hides an element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Hiding {
    /// `display: none` or `visibility: hidden`: the element takes no part in rendering.
    /// This is what Readability.js checks.
    Removed,
    /// The element is laid out but its text cannot be seen (zero opacity or font size,
    /// transparent color, off-screen, clipped, scaled to nothing, ...).
    Visual,
}

/// Returns `true` if an inline `style` declaration block makes the element's content invisible.
#[cfg(test)]
fn style_hides_content(style: &str) -> bool {
    style_hiding(style).is_some()
}

/// Returns how an inline `style` declaration block hides the element's content, if it does.
fn style_hiding(style: &str) -> Option<Hiding> {
    declarations_hiding(&parse_declarations(style))
}

/// A list of `(property, value)` pairs, both lower-cased and trimmed, without `!important`.
type Declarations = Vec<(String, String)>;

fn parse_declarations(style: &str) -> Declarations {
    let style = strip_css_comments(style);
    style
        .split(';')
        .filter_map(|decl| {
            let (name, value) = decl.split_once(':')?;
            let name = name.trim().to_ascii_lowercase();
            let value = value.trim().to_ascii_lowercase();
            let value = value
                .strip_suffix("!important")
                .map(str::trim_end)
                .unwrap_or(&value)
                .to_string();
            if name.is_empty() || value.is_empty() {
                return None;
            }
            Some((name, value))
        })
        .collect()
}

fn strip_css_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        match rest[start + 2..].find("*/") {
            Some(end) => rest = &rest[start + 2 + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

fn declarations_hide_content(decls: &Declarations) -> bool {
    declarations_hiding(decls).is_some()
}

fn declarations_hiding(decls: &Declarations) -> Option<Hiding> {
    let mut positioned = false;
    let mut offscreen = false;
    let mut overflow_hidden = false;
    let mut zero_box = false;

    for (name, value) in decls {
        match name.as_str() {
            "display" if value == "none" => return Some(Hiding::Removed),
            "visibility" if value == "hidden" || value == "collapse" => {
                return Some(Hiding::Removed)
            }
            "content-visibility" if value == "hidden" => return Some(Hiding::Visual),
            "opacity" if parse_alpha(value).map_or(false, |a| a < 0.1) => {
                return Some(Hiding::Visual)
            }
            "font-size" if is_unreadable_font_size(value) => return Some(Hiding::Visual),
            "color" if is_transparent_color(value) => return Some(Hiding::Visual),
            "text-indent" if is_far_offset(value) => return Some(Hiding::Visual),
            "clip" if is_zero_clip_rect(value) => return Some(Hiding::Visual),
            "clip-path" if is_hiding_clip_path(value) => return Some(Hiding::Visual),
            "transform" if is_zero_scale(value) => return Some(Hiding::Visual),
            "filter" if is_zero_filter_opacity(value) => return Some(Hiding::Visual),
            "position" if value == "absolute" || value == "fixed" => positioned = true,
            "left" | "top" | "right" | "bottom" | "margin-left" | "margin-top" | "inset" => {
                offscreen |= value.split_ascii_whitespace().any(is_far_offset)
            }
            "overflow" | "overflow-x" | "overflow-y" => {
                overflow_hidden |= value
                    .split_ascii_whitespace()
                    .any(|v| v == "hidden" || v == "clip")
            }
            "width" | "height" | "max-width" | "max-height" => {
                zero_box |= parse_length(value).map_or(false, |(n, unit)| {
                    n <= 0.0 || (n <= 1.0 && (unit == "px" || unit == "pt"))
                })
            }
            _ => {}
        }
    }

    if (positioned && offscreen) || (overflow_hidden && zero_box) {
        Some(Hiding::Visual)
    } else {
        None
    }
}

/// Parses a `<number>` or `<percentage>` alpha-like value into the 0..=1 range.
fn parse_alpha(value: &str) -> Option<f64> {
    if let Some(pct) = value.strip_suffix('%') {
        return pct.trim().parse::<f64>().ok().map(|p| p / 100.0);
    }
    value.parse::<f64>().ok()
}

/// Splits a `<length>` like `-9999px` or `0.5em` into `(number, unit)`. Bare `0` has an empty unit.
fn parse_length(value: &str) -> Option<(f64, &str)> {
    let value = value.trim();
    let split = value
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E')))
        .unwrap_or(value.len());
    // `e` is a valid exponent char but also the start of `em`; retry without it when needed.
    let (num, unit) = match value[..split].parse::<f64>() {
        Ok(n) => (n, &value[split..]),
        Err(_) => {
            let split = value
                .find(|c: char| !(c.is_ascii_digit() || matches!(c, '-' | '+' | '.')))
                .unwrap_or(value.len());
            (value[..split].parse::<f64>().ok()?, &value[split..])
        }
    };
    Some((num, unit.trim()))
}

fn is_unreadable_font_size(value: &str) -> bool {
    match parse_length(value) {
        Some((n, _)) if n <= 0.0 => true,
        Some((n, unit)) => n <= 1.0 && (unit == "px" || unit == "pt"),
        None => false,
    }
}

/// An offset large enough to push an element (or its first line) out of the viewport.
fn is_far_offset(value: &str) -> bool {
    match parse_length(value) {
        Some((n, "px")) | Some((n, "pt")) => n <= -999.0,
        Some((n, "")) => n <= -999.0,
        Some((n, _)) => n <= -100.0,
        None => false,
    }
}

fn is_transparent_color(value: &str) -> bool {
    if value == "transparent" {
        return true;
    }
    if let Some(hex) = value.strip_prefix('#') {
        return match hex.len() {
            4 => hex.ends_with('0'),
            8 => hex.ends_with("00"),
            _ => false,
        };
    }
    for func in ["rgba", "hsla", "rgb", "hsl"] {
        if let Some(args) = function_args(value, func) {
            let parts: Vec<&str> = args
                .split(|c: char| c == ',' || c == '/' || c.is_ascii_whitespace())
                .filter(|p| !p.is_empty())
                .collect();
            return parts.len() == 4 && parse_alpha(parts[3]).map_or(false, |a| a <= 0.0);
        }
    }
    false
}

/// `clip: rect(0 0 0 0)` and `clip: rect(1px, 1px, 1px, 1px)` are the classic "visually hidden" recipe.
fn is_zero_clip_rect(value: &str) -> bool {
    let Some(args) = function_args(value, "rect") else {
        return false;
    };
    let parts: Vec<&str> = args
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    parts.len() == 4
        && parts
            .iter()
            .all(|p| parse_length(p).map_or(false, |(n, _)| n <= 1.0))
}

fn is_hiding_clip_path(value: &str) -> bool {
    if let Some(args) = function_args(value, "inset") {
        return args
            .split_ascii_whitespace()
            .next()
            .and_then(parse_length)
            .map_or(false, |(n, unit)| unit == "%" && n >= 50.0);
    }
    if let Some(args) = function_args(value, "circle") {
        return args
            .split_ascii_whitespace()
            .next()
            .and_then(parse_length)
            .map_or(false, |(n, _)| n <= 0.0);
    }
    false
}

fn is_zero_scale(value: &str) -> bool {
    for func in ["scale3d", "scalex", "scaley", "scale"] {
        if let Some(args) = function_args(value, func) {
            let mut factors = args
                .split(|c: char| c == ',' || c.is_ascii_whitespace())
                .filter(|p| !p.is_empty())
                .filter_map(parse_alpha);
            // `scale3d` keeps text visible as long as both x and y are non-zero.
            return match func {
                "scale3d" => factors.take(2).any(|f| f.abs() < 0.05),
                _ => factors.any(|f| f.abs() < 0.05),
            };
        }
    }
    false
}

fn is_zero_filter_opacity(value: &str) -> bool {
    function_args(value, "opacity")
        .and_then(|args| parse_alpha(args.trim()))
        .map_or(false, |a| a < 0.1)
}

/// Returns the argument list of `func(...)` if `value` contains that function call.
fn function_args<'a>(value: &'a str, func: &str) -> Option<&'a str> {
    let start = value.find(func)?;
    // Make sure we matched a whole function name (`scale(` and not `scalex(`).
    let after = &value[start + func.len()..];
    let after = after.trim_start();
    let inner = after.strip_prefix('(')?;
    let end = inner.find(')')?;
    Some(&inner[..end])
}

/// Upper bound on the stylesheet text that is scanned; anything beyond is ignored.
const MAX_CSS_LEN: usize = 2 * 1024 * 1024;
/// Upper bound on the number of parsed rules.
const MAX_RULES: usize = 20_000;
/// Upper bound on the number of hiding selectors that are matched against the document.
const MAX_HIDING_SELECTORS: usize = 2_000;
/// Nesting depth of at-rules beyond which the scanner stops descending.
const MAX_AT_RULE_DEPTH: usize = 16;
/// Selectors are matched in batches of this size, one tree traversal per batch.
const SELECTOR_BATCH: usize = 64;

/// Scans the document's `<style>` elements for simple rules that hide their subjects
/// (for example `.sr-only { position: absolute; clip: rect(0 0 0 0); ... }` or
/// `#promo { display: none }`) and marks every matching element that contains text with
/// the `hidden` attribute.
///
/// Only unconditional selectors made of tag names, classes and ids (`p.note`, `.a.b`, `#x`)
/// are honored. Rules inside at-rules that do not apply to every screen reader
/// (`@media print`, `@media (max-width: ...)`, ...) are skipped.
///
/// Must run before `<style>` elements are removed from the document.
pub(crate) fn apply_stylesheet_hiding(doc: &Document) {
    let mut css = String::new();
    for style in doc.select("style").nodes() {
        if css.len() >= MAX_CSS_LEN {
            break;
        }
        css.push_str(&style.text());
        css.push('\n');
    }
    if css.is_empty() {
        return;
    }
    if css.len() > MAX_CSS_LEN {
        let mut cut = MAX_CSS_LEN;
        while !css.is_char_boundary(cut) {
            cut -= 1;
        }
        css.truncate(cut);
    }

    let mut selectors = hiding_selectors(&css);
    selectors.truncate(MAX_HIDING_SELECTORS);

    for batch in selectors.chunks(SELECTOR_BATCH) {
        // Selectors are validated individually so one bad apple does not spoil the batch.
        let valid: Vec<&str> = batch
            .iter()
            .map(String::as_str)
            .filter(|s| Matcher::new(s).is_ok())
            .collect();
        if valid.is_empty() {
            continue;
        }
        let Ok(matcher) = Matcher::new(&valid.join(", ")) else {
            continue;
        };
        for node in doc.select_matcher(&matcher).nodes() {
            if has_text(node) {
                node.set_attr("hidden", "");
            }
        }
    }
}

/// Properties that take part in hiding decisions. A conditional rule (for example inside
/// `@media (min-width: ...)`) that sets one of them without hiding is treated as revealing
/// its subject under some circumstances, so the subject is kept.
#[rustfmt::skip]
static VISIBILITY_PROPERTIES: phf::Set<&'static str> = phf_set!(
    "display", "visibility", "content-visibility", "opacity", "font-size", "color",
    "text-indent", "clip", "clip-path", "transform", "filter", "position",
    "left", "top", "right", "bottom", "margin-left", "margin-top", "inset",
    "overflow", "overflow-x", "overflow-y", "width", "height", "max-width", "max-height",
);

/// Returns the simple selectors whose merged declarations hide their subjects.
fn hiding_selectors(css: &str) -> Vec<String> {
    let rules = collect_rules(&strip_css_comments(css));

    // Later declarations for the same selector override earlier ones.
    let mut merged: HashMap<&str, Declarations> = HashMap::default();
    let mut order: Vec<&str> = vec![];
    let mut revealed: HashSet<&str> = HashSet::default();
    for rule in &rules {
        if rule.conditional {
            let touches_visibility = rule
                .declarations
                .iter()
                .any(|(n, _)| VISIBILITY_PROPERTIES.contains(n.as_str()));
            if touches_visibility && !declarations_hide_content(&rule.declarations) {
                revealed.insert(rule.selector.as_str());
            }
            continue;
        }
        let entry = merged.entry(rule.selector.as_str()).or_insert_with(|| {
            order.push(&rule.selector);
            vec![]
        });
        for (name, value) in rule.declarations.iter() {
            entry.retain(|(n, _)| n != name);
            entry.push((name.clone(), value.clone()));
        }
    }

    order
        .into_iter()
        .filter(|selector| {
            let decls = &merged[*selector];
            if !is_simple_selector(selector)
                || revealed.contains(*selector)
                || !declarations_hide_content(decls)
            {
                return false;
            }
            // `font-size: 0` on a container with explicitly sized descendants is a common
            // layout trick (inline-block gaps), not a hiding technique.
            let hides_by_font_size_only = decls
                .iter()
                .any(|(n, v)| n == "font-size" && is_unreadable_font_size(v))
                && !declarations_hide_content(
                    &decls
                        .iter()
                        .filter(|(n, _)| n != "font-size")
                        .cloned()
                        .collect(),
                );
            if hides_by_font_size_only {
                let descendants_resized = merged.iter().any(|(other, other_decls)| {
                    *other != *selector
                        && other.starts_with(*selector)
                        && other_decls
                            .iter()
                            .any(|(n, v)| n == "font-size" && !is_unreadable_font_size(v))
                });
                if descendants_resized {
                    return false;
                }
            }
            true
        })
        .map(str::to_string)
        .collect()
}

struct Rule {
    selector: String,
    /// Shared between the selectors of one `a, b, c { ... }` block.
    declarations: Rc<Declarations>,
    /// The rule sits inside an at-rule that does not apply to every screen reader
    /// (`@media print`, `@media (min-width: ...)`, ...).
    conditional: bool,
}

/// Flattens a stylesheet into rules, descending into at-rule blocks.
fn collect_rules(css: &str) -> Vec<Rule> {
    let mut rules = vec![];
    collect_rules_into(css, false, 0, &mut rules);
    rules
}

fn collect_rules_into(css: &str, conditional: bool, depth: usize, rules: &mut Vec<Rule>) {
    if depth > MAX_AT_RULE_DEPTH {
        return;
    }
    let mut rest = css;
    loop {
        if rules.len() >= MAX_RULES {
            return;
        }
        let Some(open) = rest.find('{') else {
            return;
        };
        let prelude = rest[..open].trim();
        let Some(close) = matching_brace(&rest[open..]) else {
            return;
        };
        let body = &rest[open + 1..open + close];
        rest = &rest[open + close + 1..];

        // A statement at-rule (`@import ...;`) may precede the prelude; drop it.
        let prelude = prelude.rsplit(';').next().unwrap_or(prelude).trim();

        if let Some(at_rule) = prelude.strip_prefix('@') {
            let conditional = conditional || !at_rule_applies_to_screen(at_rule);
            collect_rules_into(body, conditional, depth + 1, rules);
            continue;
        }

        let decls = parse_declarations(body);
        if decls.is_empty() {
            continue;
        }
        let decls = Rc::new(decls);
        for selector in prelude.split(',') {
            if rules.len() >= MAX_RULES {
                return;
            }
            let selector = selector.trim();
            if !selector.is_empty() {
                rules.push(Rule {
                    selector: selector.to_string(),
                    declarations: Rc::clone(&decls),
                    conditional,
                });
            }
        }
    }
}

/// Returns the index (relative to `s`, which starts with `{`) of the brace closing it.
fn matching_brace(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn at_rule_applies_to_screen(at_rule: &str) -> bool {
    let at_rule = at_rule.to_ascii_lowercase();
    let name = at_rule
        .split(|c: char| c.is_ascii_whitespace() || c == '(')
        .next()
        .unwrap_or("");
    match name {
        "media" => ![
            "print",
            "speech",
            "aural",
            "braille",
            "embossed",
            "handheld",
            "projection",
            "tty",
            "tv",
            "not",
            "min-",
            "max-",
            "prefers-",
            "orientation",
            "hover",
            "pointer",
            "aspect-ratio",
            "resolution",
        ]
        .iter()
        .any(|kw| at_rule.contains(kw)),
        "supports" | "layer" | "scope" | "container" => true,
        _ => false,
    }
}

/// A selector that applies unconditionally and that we can hand to `dom_query`:
/// an optional tag name plus at least one class or id, no combinators, no pseudo-classes,
/// no attribute selectors.
fn is_simple_selector(selector: &str) -> bool {
    !selector.is_empty()
        && selector.len() < 200
        && selector
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '#' | '-' | '_'))
        && selector.contains(['.', '#'])
}

/// Removes element content that browsers never render: fallback children of media and
/// widget elements, closed dialogs, datalists, ruby parentheses and SVG/MathML metadata.
pub(crate) fn remove_unrendered_content(doc: &Document) {
    doc.select("datalist, dialog:not([open]), rp, svg title, svg desc, svg metadata, annotation, annotation-xml")
        .remove();

    for node in doc.select("video, audio, canvas, meter, progress").nodes() {
        for child in node.children() {
            let keep = child
                .node_name()
                .map_or(false, |n| matches!(n.as_ref(), "source" | "track"));
            if !keep {
                child.remove_from_parent();
            }
        }
    }
}

/// Characters that render as nothing but survive into plain text: soft hyphen,
/// zero-width space, word joiner, invisible operators, BOM, bidi embedding/override/isolate
/// controls, interlinear annotation marks and the Unicode "tag" characters used for
/// invisible ASCII smuggling. Zero-width (non-)joiners and LRM/RLM are kept because they
/// carry meaning in several scripts and in emoji sequences.
fn is_invisible_char(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{180E}'
            | '\u{200B}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{E0000}'..='\u{E007F}'
    )
}

/// Returns the text without invisible characters (see [`is_invisible_char`]).
pub(crate) fn strip_invisible_chars(text: StrTendril) -> StrTendril {
    if !text.chars().any(is_invisible_char) {
        return text;
    }
    let mut out = StrTendril::with_capacity(text.len32());
    for c in text.chars().filter(|c| !is_invisible_char(*c)) {
        out.push_char(c);
    }
    out
}

/// Removes invisible characters (see [`is_invisible_char`]) from every text node under `root`.
pub(crate) fn strip_invisible_chars_in_tree(root: &Node) {
    for node in text_nodes(root) {
        node.update(|tree_node| {
            if let NodeData::Text { ref mut contents } = tree_node.data {
                if contents.chars().any(is_invisible_char) {
                    let cleaned: StrTendril = strip_invisible_chars(contents.clone());
                    *contents = cleaned;
                }
            }
        });
    }
}

/// [`strip_invisible_chars`] for owned strings.
pub(crate) fn strip_invisible_chars_string(text: String) -> String {
    if !text.chars().any(is_invisible_char) {
        return text;
    }
    text.chars().filter(|c| !is_invisible_char(*c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_styles_that_hide() {
        for style in [
            "display:none",
            "DISPLAY: NONE",
            "display:/* x */none",
            "display: none !important",
            "visibility:hidden",
            "visibility: collapse",
            "content-visibility: hidden",
            "opacity:0",
            "opacity: .01",
            "opacity: 0%",
            "font-size:0",
            "font-size: 0px",
            "font-size: 1px",
            "color: transparent",
            "color: rgba(0,0,0,0)",
            "color: rgb(0 0 0 / 0)",
            "color: hsla(0, 0%, 0%, 0)",
            "color: #00000000",
            "color: #0000",
            "position:absolute;left:-9999px",
            "position: fixed; top: -100vh",
            "position:absolute;inset:-9999px auto auto -9999px",
            "height:0;overflow:hidden",
            "width: 1px; height: 1px; overflow: hidden",
            "max-height: 0; overflow-y: hidden",
            "position:absolute;clip:rect(0 0 0 0)",
            "clip: rect(1px, 1px, 1px, 1px)",
            "clip-path: inset(100%)",
            "clip-path: inset(50%)",
            "clip-path: circle(0)",
            "transform: scale(0)",
            "transform: translateX(10px) scale(0, 1)",
            "transform: scaleY(0)",
            "transform: scale3d(0, 1, 1)",
            "filter: opacity(0)",
            "text-indent: -9999px",
            "text-indent: -100%",
        ] {
            assert!(style_hides_content(style), "should hide: {style}");
        }
    }

    #[test]
    fn inline_styles_that_do_not_hide() {
        for style in [
            "display:block",
            "display: inline-block; color: red",
            "visibility: visible",
            "opacity: 0.5",
            "opacity: 1",
            "font-size: 12px",
            "font-size: 1.2em",
            "font-size: 0.9rem",
            "color: #000",
            "color: rgba(0,0,0,0.8)",
            "color: rgb(255, 255, 255)",
            "left:-9999px",
            "position: relative; left: -10px",
            "position: absolute; top: 0",
            "height: 0; overflow: visible",
            "height: 0",
            "overflow: hidden",
            "overflow: hidden; height: 300px",
            "clip-path: inset(10%)",
            "transform: scale(1.5)",
            "transform: scale3d(1, 1, 0)",
            "transform: translateX(-9999px)",
            "text-indent: 2em",
            "background: url(data:image/png;base64,AAAA)",
            "",
            "garbage",
        ] {
            assert!(!style_hides_content(style), "should not hide: {style}");
        }
    }

    #[test]
    fn stylesheet_selectors() {
        let css = r#"
            @import url(x.css);
            /* comment { .nope { display:none } } */
            .srhide { display: none; }
            .vh, p.offscreen { position:absolute; width:1px; height:1px; overflow:hidden; clip:rect(0 0 0 0); }
            #cssid { visibility: hidden }
            .shown { display: none }
            .shown { display: block }
            .a .b { display: none }
            .c:hover { display: none }
            [data-x] { display: none }
            p { display: none }
            @media print { .print-only-hidden { display: none } }
            @media screen { .screen-hidden { display: none } }
            @media (max-width: 600px) { .mobile-hidden { display: none } }
            .responsive { display: none }
            @media (min-width: 740px) { .responsive { display: inline } }
            .print-recolored { display: none }
            @media print { .print-recolored { color: black } }
            @keyframes fade { from { opacity: 0 } to { opacity: 1 } }
            @supports (display: grid) { .grid-hidden { opacity: 0 } }
            .gap { font-size: 0 }
            .gap li { font-size: 16px }
            .tiny { font-size: 0 }
        "#;
        let selectors = hiding_selectors(css);
        assert_eq!(
            selectors,
            vec![
                ".srhide",
                ".vh",
                "p.offscreen",
                "#cssid",
                ".screen-hidden",
                ".grid-hidden",
                ".tiny",
            ]
        );
    }

    #[test]
    fn deeply_nested_at_rules_do_not_overflow() {
        let depth = 100_000;
        let mut css = String::new();
        for _ in 0..depth {
            css.push_str("@media screen{");
        }
        css.push_str(".x{display:none}");
        for _ in 0..depth {
            css.push('}');
        }
        assert!(hiding_selectors(&css).is_empty());
        let mut unbalanced = String::new();
        for _ in 0..depth {
            unbalanced.push_str("}}{{");
        }
        assert!(hiding_selectors(&unbalanced).is_empty());
    }

    #[test]
    fn rule_cap_holds_for_one_giant_selector_list() {
        let mut css = String::new();
        for i in 0..(MAX_RULES * 2) {
            css.push_str(&format!(".c{i},"));
        }
        css.push_str(".last{display:none}");
        assert_eq!(collect_rules(&css).len(), MAX_RULES);
    }

    #[test]
    fn invisible_chars() {
        let text = StrTendril::from("ab\u{200B}c\u{E0049}\u{E0047}\u{202E}d\u{FEFF}");
        assert_eq!(strip_invisible_chars(text).as_ref(), "abcd");
        // ZWJ, ZWNJ and RLM are meaningful and must survive.
        let text = StrTendril::from("👨\u{200D}👩 ب\u{200C}ت \u{200F}");
        assert_eq!(strip_invisible_chars(text.clone()), text);
    }
}
