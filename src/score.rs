use dom_query::{Node, NodeData};
use html5ever::local_name;

use crate::glob::*;

pub(crate) fn get_node_score(node: &Node) -> f32 {
    let score = node.attr(SCORE_ATTR);
    if let Some(Ok(score)) = score.map(|s| s.parse::<f32>()) {
        return score;
    }
    0.0
}

pub(crate) fn has_node_score(node: &Node) -> bool {
    node.has_attr(SCORE_ATTR)
}

pub(crate) fn set_node_score(node: &Node, score: f32) {
    node.set_attr(SCORE_ATTR, &score.to_string());
}

pub(crate) fn init_node_score(node: &Node, weigh_classes: bool) -> f32 {
    let score = determine_node_score(node, weigh_classes);
    set_node_score(node, score);
    score
}

pub(crate) fn determine_node_score(node: &Node, weigh_classes: bool) -> f32 {
    let Some(node_name) = node.node_name() else {
        return 0.0;
    };

    let score: f32 = match node_name.as_ref() {
        "div" => 5.0,
        "pre" | "td" | "blockquote" => 3.0,
        "address" | "ol" | "ul" | "dl" | "dd" | "dt" | "li" | "form" => -3.0,
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "th" => -5.0,
        _ => 0.0,
    };

    score + get_class_weight(node, weigh_classes)
}

pub(crate) fn get_class_weight(node: &Node, weigh_classes: bool) -> f32 {
    let mut weight = 0.0;

    if !weigh_classes {
        return weight;
    }

    node.query(|n| {
        if let NodeData::Element(ref el) = n.data {
            if let Some(a) = el
                .attrs
                .iter()
                .find(|a| a.name.local == local_name!("class"))
            {
                let class_name = &a.value.to_lowercase();
                if RX_CLASSES_NEGATIVE.is_match(class_name) {
                    weight -= 25.0;
                }
                if CLASSES_NEGATIVE.iter().any(|pat| class_name.contains(pat)) {
                    weight -= 25.0;
                }
                if CLASSES_POSITIVE.iter().any(|pat| class_name.contains(pat)) {
                    weight += 25.0;
                }
            };

            if let Some(a) = el.attrs.iter().find(|a| a.name.local == local_name!("id")) {
                let id = &a.value.to_lowercase();
                if RX_CLASSES_NEGATIVE.is_match(id) {
                    weight -= 25.0;
                }
                if CLASSES_NEGATIVE.iter().any(|pat| id.contains(pat)) {
                    weight -= 25.0;
                }
                if CLASSES_POSITIVE.iter().any(|pat| id.contains(pat)) {
                    weight += 25.0;
                }
            }
        }
    });

    weight
}
