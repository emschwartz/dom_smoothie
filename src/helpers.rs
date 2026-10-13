use std::collections::HashSet;

use foldhash::HashMap;
use unicode_segmentation::UnicodeSegmentation;

use dom_query::{local_name, NodeData, NodeId, NodeRef, Selection};

use tendril::StrTendril;

use crate::glob::{MINI_ARIA_HIDDEN, MINI_FALLBACK_IMG, PHRASING_ELEMS};
use crate::matching::is_invisible_style;

pub(crate) fn text_similarity(text_a: &str, text_b: &str) -> f64 {
    if text_a.is_empty() || text_b.is_empty() {
        return 0.0;
    }

    let a = text_a.to_lowercase();
    let b = text_b.to_lowercase();

    if a.contains(&b) {
        return 1.0;
    }

    let tokens_a: HashSet<&str> = a.unicode_words().collect();
    let tokens_b: Vec<&str> = b.unicode_words().collect();

    let b_chars_total: usize = tokens_b.iter().map(|s| s.chars().count()).sum();
    let b_chars_unique: usize = tokens_b
        .iter()
        .filter(|&&s| !tokens_a.contains(s))
        .map(|s| s.chars().count())
        .sum();

    let distance_b = b_chars_unique as f64 / b_chars_total as f64;
    1.0 - distance_b
}

pub(crate) fn is_phrasing_content(node: &NodeRef) -> bool {
    if node.is_text() {
        return true;
    }

    // only elements has a node name
    let Some(qual_name) = node.qual_name_ref() else {
        return false;
    };
    let node_name = qual_name.local.as_ref();
    if PHRASING_ELEMS.contains(node_name) {
        return true;
    }

    if matches!(node_name, "a" | "del" | "ins") {
        // There is no big sense to consider link content as phrasing content if they doesn't have children element.
        let children = node.children();
        return !children.is_empty() && children.into_iter().all(|n| is_phrasing_content(&n));
    }
    false
}

pub(crate) fn is_whitespace(node: &NodeRef) -> bool {
    if node.is_text() && !node.is_nonempty_text() {
        return true;
    }
    // only an element node has a node_name
    node.has_name("br")
}

pub(crate) fn has_ancestor<F>(node: &NodeRef, max_depth: Option<usize>, filter_fn: F) -> bool
where
    F: Fn(&NodeRef) -> bool,
{
    let max_depth = max_depth.map(|max_depth| if max_depth == 0 { 3 } else { max_depth });
    node.ancestors_it(max_depth).any(|a| filter_fn(&a))
}

pub(crate) fn text_density(node: &NodeRef, selector: &str, char_count: Option<usize>) -> f32 {
    let sel = Selection::from(*node).select(selector);
    let sel_nodes = sel.nodes();

    let children_length: f32 = sel_nodes
        .iter()
        .map(|n| n.normalized_char_count())
        .sum::<usize>() as f32;

    if children_length == 0.0 {
        return 0.0;
    }
    let text_length = char_count.unwrap_or_else(|| node.normalized_char_count()) as f32;

    if text_length == 0.0 {
        return 0.0;
    }
    children_length / text_length
}

pub(crate) fn normalize_spaces(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut iter = text.split_whitespace();

    if let Some(first) = iter.next() {
        result.push_str(first);
        for word in iter {
            result.push(' ');
            result.push_str(word);
        }
    }
    result
}

/// Weight of a link's text in the link density: in-page anchors count for less.
pub(crate) fn link_coeff(href: &str) -> f32 {
    if href.len() > 1 && href.starts_with('#') {
        0.3
    } else {
        1.0
    }
}

pub(crate) fn link_density(node: &NodeRef, char_count: Option<usize>) -> f32 {
    let mut link_length = 0f32;

    for a in node.find_descendants("a") {
        link_length += a.normalized_char_count() as f32 * link_coeff(&a.attr_or("href", ""));
    }

    if link_length == 0.0 {
        return 0.0;
    }

    let text_length = char_count.unwrap_or_else(|| node.normalized_char_count()) as f32;
    if text_length == 0.0 {
        return 0.0;
    }

    link_length / text_length
}

/// Returns the child element if the node contains exactly one element child with the given tag
/// and no non-empty text nodes.
pub(crate) fn single_child_element<'a>(node: &NodeRef<'a>, tag: &str) -> Option<NodeRef<'a>> {
    // There should be exactly 1 element child with given tag
    let children = node.element_children();
    if children.len() != 1 {
        return None;
    }

    let first = children.first()?;

    if !first.has_name(tag) {
        return None;
    }
    if node.children_it(false).any(|n| n.is_nonempty_text()) {
        return None;
    }
    Some(*first)
}

pub(crate) fn is_element_without_content(node: &NodeRef) -> bool {
    // since this function calls only for elements check `node.is_element()` is redundant
    let has_text = node.descendants_it().any(|n| n.is_nonempty_text());
    if has_text {
        return false;
    }

    let el_children_count = node.children_it(false).filter(|n| n.is_element()).count();
    if el_children_count == 0 {
        return true;
    }

    let line_breaks = node.find_descendants("br").len() + node.find_descendants("hr").len();
    el_children_count == line_breaks
}

pub(crate) fn get_dir_attr(node: &NodeRef) -> Option<String> {
    if let Some(first_child) = node.first_child() {
        if let Some(dir_attr) = first_child.attr("dir") {
            return Some(dir_attr.to_string());
        }

        let dir_attr = first_child.ancestors_it(None).find_map(|a| a.attr("dir"));
        if let Some(dir_attr) = dir_attr {
            return Some(dir_attr.to_string());
        }
    }
    None
}

pub(crate) fn node_name_in(node: &NodeRef, names: &phf::Set<&str>) -> bool {
    node.qual_name_ref()
        .is_some_and(|name| names.contains(name.local.as_ref()))
}

pub(crate) fn is_probably_visible(node: &NodeRef) -> bool {
    if node.has_attr("hidden") {
        return false;
    }
    if is_invisible_style(node) {
        return false;
    }
    !MINI_ARIA_HIDDEN.match_node(node) || MINI_FALLBACK_IMG.match_node(node)
}

pub(crate) fn get_node_matching_string(node: &NodeRef) -> StrTendril {
    let mut buf = StrTendril::new();
    let Some(el) = node.element_ref() else {
        return buf;
    };

    for attr in &el.attrs {
        if !matches!(attr.name.local.as_ref(), "class" | "id") {
            continue;
        }
        buf.push_tendril(&attr.value);
        buf.push_char(' ');
    }

    buf.make_ascii_lowercase();
    buf
}

pub(crate) fn next_child_or_sibling<'a>(
    node: &NodeRef<'a>,
    ignore_child: bool,
) -> Option<NodeRef<'a>> {
    if !ignore_child {
        if let Some(first_child) = node.first_element_child() {
            return Some(first_child);
        }
    }

    if let Some(sibling) = node.next_element_sibling() {
        return Some(sibling);
    }
    let mut parent = node.parent();
    while let Some(parent_node) = parent {
        if let Some(next_sibling) = parent_node.next_element_sibling() {
            return Some(next_sibling);
        }
        parent = parent_node.parent();
    }
    None
}

#[cfg(not(feature = "aho-corasick"))]
/// A lightweight byte-level pre-checker used to quickly skip patterns
/// that cannot occur in the haystack.
pub(crate) struct BytePatternCheck<'a> {
    haystack: &'a str,
    char_map: [u8; 256],
}

#[cfg(not(feature = "aho-corasick"))]
impl<'a> BytePatternCheck<'a> {
    pub(crate) fn new(haystack: &'a str) -> Self {
        let mut char_map = [0u8; 256];

        for &b in haystack.as_bytes() {
            char_map[b as usize] = 1;
        }
        Self { haystack, char_map }
    }
    #[inline]
    fn pre_check(&self, pat: &str) -> bool {
        for &b in pat.as_bytes() {
            if self.char_map[b as usize] == 0 {
                return false;
            }
        }
        true
    }
    /// Checks if the haystack contains any of the given patterns.
    /// Performs a cheap bitmap pre-check before `str::contains`.
    pub(crate) fn contains_any(&self, pats: &[&str]) -> bool {
        pats.iter()
            .any(|pat| self.pre_check(pat) && self.haystack.contains(pat))
    }
}

/// `normalized_char_count` and [`link_density`] of every node under a root,
/// from one bottom-up pass instead of a subtree walk per lookup.
pub(crate) struct SubtreeStats {
    stats: HashMap<NodeId, TextStat>,
}

#[derive(Clone, Copy, Default)]
struct TextStat {
    chars: u32,
    /// Weighted chars of the `<a>` descendants, as `link_density` sums them.
    link_chars: f32,
    has_text: bool,
    leading_ws: bool,
    trailing_ws: bool,
}

impl TextStat {
    fn of_text(text: &str) -> Self {
        let Some(first) = text.chars().next() else {
            return Self::default();
        };
        let mut chars = 0u32;
        let mut prev_ws = true;
        for c in text.chars() {
            let ws = c.is_whitespace();
            if !(prev_ws && ws) {
                chars += 1;
            }
            prev_ws = ws;
        }
        if prev_ws && chars > 0 {
            chars -= 1;
        }
        Self {
            chars,
            has_text: true,
            leading_ws: first.is_whitespace(),
            trailing_ws: prev_ws,
            ..Self::default()
        }
    }

    /// `self`'s text followed by `next`'s, in document order.
    fn then(self, next: Self) -> Self {
        let link_chars = self.link_chars + next.link_chars;
        let text = if !self.has_text {
            next
        } else if !next.has_text {
            self
        } else {
            // text on both sides of the boundary keeps one space between them
            let sep = u32::from(
                self.chars > 0 && next.chars > 0 && (self.trailing_ws || next.leading_ws),
            );
            Self {
                chars: self.chars + next.chars + sep,
                has_text: true,
                leading_ws: self.leading_ws,
                trailing_ws: next.trailing_ws,
                ..Self::default()
            }
        };
        Self { link_chars, ..text }
    }
}

impl SubtreeStats {
    pub(crate) fn build(root: &NodeRef) -> Self {
        let nodes: Vec<NodeRef> = std::iter::once(*root)
            .chain(root.descendants_it())
            .collect();
        let mut stats: HashMap<NodeId, TextStat> =
            HashMap::with_capacity_and_hasher(nodes.len(), foldhash::fast::RandomState::default());
        // Reverse pre-order: a node's descendants are folded in before the node itself,
        // and siblings arrive last-to-first, which `then` turns back into document order.
        for node in nodes.iter().rev() {
            // a text node's own stats; an `<a>` reports its link weight for the parent
            let (text, coeff) = node.query_or((None, None), |tree_node| match &tree_node.data {
                NodeData::Text { contents } => (Some(TextStat::of_text(contents)), None),
                NodeData::Element(el) if el.name.local == local_name!("a") => {
                    let href = el
                        .attrs
                        .iter()
                        .find(|a| a.name.local == local_name!("href"))
                        .map_or("", |a| a.value.as_ref());
                    (None, Some(link_coeff(href)))
                }
                _ => (None, None),
            });
            if let Some(text) = text {
                stats.insert(node.id, text);
            }
            if node.id == root.id {
                break;
            }
            if let Some(parent) = node.parent() {
                let mut child = stats.get(&node.id).copied().unwrap_or_default();
                if let Some(coeff) = coeff {
                    child.link_chars += child.chars as f32 * coeff;
                }
                let acc = stats.entry(parent.id).or_default();
                *acc = child.then(*acc);
            }
        }
        Self { stats }
    }

    pub(crate) fn char_count(&self, node: &NodeRef) -> usize {
        self.stats.get(&node.id).map_or(0, |s| s.chars as usize)
    }

    pub(crate) fn link_density(&self, node: &NodeRef) -> f32 {
        let Some(s) = self.stats.get(&node.id) else {
            return 0.0;
        };
        if s.link_chars == 0.0 || s.chars == 0 {
            return 0.0;
        }
        s.link_chars / s.chars as f32
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    // `SubtreeStats` must agree with the per-node walks it replaces.
    #[test]
    fn test_subtree_stats_match_per_node_walks() {
        use dom_query::Document;
        let mut pages = 0;
        for entry in std::fs::read_dir("test-pages/ok").unwrap() {
            let path = entry.unwrap().path().join("source.html");
            let html = std::fs::read_to_string(&path).unwrap();
            pages += 1;
            let doc = Document::from(html);
            let body = doc.select_single("body");
            let body = body.nodes().first().unwrap();
            let stats = SubtreeStats::build(body);
            for node in std::iter::once(*body).chain(body.descendants_it()) {
                if !node.is_element() {
                    continue;
                }
                assert_eq!(
                    stats.char_count(&node),
                    node.normalized_char_count(),
                    "char count of {:?} in {}",
                    node.node_name(),
                    path.display()
                );
                let expected = link_density(&node, None);
                let got = stats.link_density(&node);
                assert!(
                    (got - expected).abs() <= 1e-5 * expected.max(1.0),
                    "link density of {:?} in {}: {got} vs {expected}",
                    node.node_name(),
                    path.display()
                );
            }
        }
        assert!(pages > 0);
    }

    const FLOAT_TOLERANCE: f64 = 0.00001;

    #[test]
    fn test_text_similarity() {
        let text_a = "The quick brown fox";
        let text_b = "The quick fox";
        let similarity = text_similarity(text_a, text_b);
        assert!(similarity > 0.75);
    }

    #[test]
    fn test_text_similarity_contains() {
        let text_a = "the quick brown fox jumps over the lazy dog";
        let text_b = "The Quick Brown Fox";
        let similarity = text_similarity(text_a, text_b);
        assert!(1.0 - similarity < FLOAT_TOLERANCE);
    }

    #[test]
    fn test_text_similarity_similar() {
        let text_a = "DeepMind新电脑已可利用记忆自学 人工智能迈上新台阶_科技_腾讯网";
        let text_b = "DeepMind新电脑已可利用记忆自学 人工智能迈上新台阶";
        let similarity = text_similarity(text_a, text_b);
        assert!(1.0 - similarity < FLOAT_TOLERANCE);
    }

    #[test]
    fn test_normalize_spaces() {
        let text = "    The quick  brown  fox\n jumps over the lazy dog. ";
        let normalized_text = normalize_spaces(text);
        let expected = "The quick brown fox jumps over the lazy dog.";
        assert_eq!(expected, normalized_text);
    }

    #[test]
    #[cfg(not(feature = "aho-corasick"))]
    fn test_ascii_pattern_check() {
        let class = "article primary main äußerlich konnen";
        let check = BytePatternCheck::new(class);
        assert!(check.contains_any(&["äußerlich"]));
        assert!(check.contains_any(&["article", "post"]));
        assert!(!check.contains_any(&["können"]));
    }
}
