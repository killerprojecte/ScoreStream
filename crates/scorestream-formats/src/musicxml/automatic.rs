//! Conservative per-note selection, not parse-failure fallback. A successful
//! legacy parse is insufficient: it can flatten groups and reorder signs.
use super::xml::Node;
use scorestream_core::score::SimpleArticulation;

/// Prove that the legacy writer retains the articulation wrapper/group topology
/// and child ordering. Existing family payload validators still own semantics.
/// Notes without articulation groups retain their existing mapping contract.
pub(super) fn is_canonical_simple(note: &Node) -> bool {
    let wrappers: Vec<_> = note.children_named("notations").collect();
    if !wrappers.iter().any(|w| w.child("articulations").is_some()) {
        return true;
    }
    if wrappers.len() != 1 || !wrappers[0].attrs.is_empty() {
        return false;
    }
    let wrapper = wrappers[0];
    // Writer order is tied stop, tied start, fermata, slur, articulations.
    // More than one slur needs relation-aware ordering proof; retain it instead.
    let mut previous_rank = None;
    for child in &wrapper.children {
        // The tuplet bridge independently retains exact endpoint insertion order.
        if child.name == "tuplet" {
            continue;
        }
        let rank = match child.name.as_str() {
            "tied" if child.attr("type") == Some("stop") => 0,
            "tied" if child.attr("type") == Some("start") => 1,
            "fermata" => 2,
            "slur" => 3,
            "articulations" => 4,
            _ => return false,
        };
        if previous_rank.is_some_and(|previous| rank <= previous) {
            return false;
        }
        previous_rank = Some(rank);
        if child.name != "articulations" {
            continue;
        }
        if !child.attrs.is_empty() || child.children.is_empty() {
            return false;
        }
        let mut previous_sign = None;
        for sign in &child.children {
            let Some(value) = SimpleArticulation::from_name(&sign.name) else {
                return false;
            };
            if sign.attrs.keys().any(|key| key != "placement")
                || previous_sign.is_some_and(|previous| value <= previous)
            {
                return false;
            }
            previous_sign = Some(value);
        }
    }
    true
}
