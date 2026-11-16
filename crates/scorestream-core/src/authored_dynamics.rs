//! Lossless authored direction statements, independent from unique executable sound state.
use crate::{Diagnostic, Document, DocumentKind, Mark, Rational, Record, Scalar, SourceRange};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const EXTENSION: &str = "scorestream.authored_dynamics@1";
pub const KIND: &str = "scorestream.authored_dynamics";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredDynamic {
    pub part: String,
    pub measure: String,
    pub order: u32,
    pub cursor: Rational,
    pub visual_at: Rational,
    pub sounding_at: Rational,
    pub sign: String,
    pub percent: Rational,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staff: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<Rational>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_sound: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound_offset: Option<Rational>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_x: Option<Rational>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_y: Option<Rational>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative_x: Option<Rational>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative_y: Option<Rational>,
}
pub type Hosts = crate::wedges::Hosts;
pub fn valid(e: &AuthoredDynamic, hosts: &Hosts) -> bool {
    let Some((start, len, staves)) = hosts.get(&(e.part.clone(), e.measure.clone())) else {
        return false;
    };
    let current = start + &e.cursor;
    let visual = &current + &e.offset.clone().unwrap_or_default();
    let sounding = if let Some(o) = &e.sound_offset {
        &current + o
    } else if e.offset_sound == Some(true) {
        visual.clone()
    } else {
        current
    };
    !e.cursor.is_negative()
        && &e.cursor <= len
        && e.visual_at == visual
        && e.sounding_at == sounding
        && e.visual_at >= *start
        && e.visual_at < start + len
        && e.sounding_at >= *start
        && e.sounding_at <= start + len
        && e.staff.is_none_or(|s| s > 0 && s <= *staves)
        && e.placement
            .as_deref()
            .is_none_or(|s| matches!(s, "above" | "below"))
        && matches!(
            e.sign.as_str(),
            "pppp"
                | "ppp"
                | "pp"
                | "p"
                | "mp"
                | "mf"
                | "f"
                | "ff"
                | "fff"
                | "ffff"
                | "sf"
                | "sfz"
                | "fp"
        )
        && !e.percent.is_negative()
        && crate::geometry::decimal(&e.percent)
        && (e.offset.is_some() || e.offset_sound.is_none())
        && [&e.default_x, &e.default_y, &e.relative_x, &e.relative_y]
            .into_iter()
            .flatten()
            .all(crate::geometry::decimal)
}
pub fn valid_set(values: &[AuthoredDynamic], hosts: &Hosts) -> bool {
    let endpoints: std::collections::BTreeSet<_> = values
        .iter()
        .filter(|e| {
            hosts
                .get(&(e.part.clone(), e.measure.clone()))
                .is_some_and(|(start, len, _)| e.sounding_at == start + len)
        })
        .map(|e| (&e.part, e.staff, &e.sounding_at))
        .collect();
    let owners: std::collections::BTreeSet<_> = values
        .iter()
        .filter(|e| endpoints.contains(&(&e.part, e.staff, &e.sounding_at)))
        .map(|e| (&e.part, &e.measure))
        .collect();
    let mut orders = BTreeMap::<(&str, &str), u32>::new();
    values.windows(2).all(|pair| {
        pair[0].part < pair[1].part
            || pair[0].part == pair[1].part
                && hosts
                    .get(&(pair[0].part.clone(), pair[0].measure.clone()))
                    .zip(hosts.get(&(pair[1].part.clone(), pair[1].measure.clone())))
                    .is_some_and(|(a, b)| a.0 <= b.0)
    }) && values.iter().all(|e| {
        let next = orders.entry((&e.part, &e.measure)).or_default();
        let ordered = e.order == *next;
        *next += 1;
        valid(e, hosts) && ordered && owners.contains(&(&e.part, &e.measure))
    })
}
pub fn hosts(doc: &Document) -> Hosts {
    let mut hosts = Hosts::new();
    for f in &doc.frames {
        for r in &f.records {
            if let Record::Measure(m) = r {
                if let Some(p) = doc.parts.iter().find(|p| p.id == m.target) {
                    hosts.insert(
                        (m.target.clone(), m.id.clone()),
                        (f.time.clone(), m.length.clone(), p.staves),
                    );
                }
            }
        }
    }
    hosts
}
pub fn encode(e: &AuthoredDynamic, index: usize) -> Mark {
    // A closed flat schema becomes individual native scalar fields, never a JSON payload.
    let object = serde_json::to_value(e).expect("typed fields");
    let mut attributes = BTreeMap::new();
    for (key, value) in object.as_object().unwrap() {
        attributes.insert(
            key.clone(),
            serde_json::from_value::<Scalar>(value.clone()).expect("flat typed scalar"),
        );
    }
    attributes.insert("index".into(), Scalar::Integer(index as i64));
    Mark {
        target: format!("#{}", e.measure),
        kind: KIND.into(),
        attributes,
        source: SourceRange::default(),
    }
}
pub fn decode(doc: &Document) -> Result<Vec<AuthoredDynamic>, Vec<Diagnostic>> {
    let fail = || {
        vec![Diagnostic::error("authored_dynamics.wire","Authored dynamics require a complete score, declared capability, exact owner/cursor/locations, ordered unique indices and closed typed fields",doc.source)]
    };
    let enabled = doc.required_extensions.iter().any(|s| s == EXTENSION);
    let hosts = hosts(doc);
    let mut entries = BTreeMap::new();
    for frame in &doc.frames {
        for r in &frame.records {
            let Record::Mark(m) = r else { continue };
            if !m.kind.starts_with(KIND) {
                continue;
            }
            if !enabled || m.kind != KIND || doc.document_kind != DocumentKind::Score {
                return Err(fail());
            }
            let mut fields = m.attributes.clone();
            let index = fields
                .remove("index")
                .and_then(|v| v.as_i64())
                .filter(|n| *n >= 0)
                .ok_or_else(fail)?;
            let e: AuthoredDynamic = serde_json::from_value(serde_json::to_value(fields).unwrap())
                .map_err(|_| fail())?;
            if m.target != format!("#{}", e.measure)
                || !valid(&e, &hosts)
                || frame.time != hosts[&(e.part.clone(), e.measure.clone())].0
            {
                return Err(fail());
            }
            if entries.insert(index, e).is_some() {
                return Err(fail());
            }
        }
    }
    if enabled && entries.is_empty() || entries.keys().enumerate().any(|(i, k)| i as i64 != *k) {
        return Err(fail());
    }
    let values: Vec<AuthoredDynamic> = entries.into_values().collect();
    if !valid_set(&values, &hosts) {
        return Err(fail());
    }
    let mut previous: BTreeMap<(&str, &str), u32> = BTreeMap::new();
    for e in &values {
        if previous
            .insert((&e.part, &e.measure), e.order)
            .is_some_and(|n| n >= e.order)
        {
            return Err(fail());
        }
    }
    Ok(values)
}
