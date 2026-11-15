//! Printed dynamics and exact numeric sound suggestions are independent authoring lanes.
use crate::{Format, MappingEffect, MappingReport};
use scorestream_core::{
    notation::{DynamicLevel, DynamicSign, PartId, Placement},
    score::*,
    *,
};
use std::collections::BTreeMap;

pub(crate) fn symbol(value: &str) -> Option<DynamicSign> {
    use DynamicLevel::*;
    Some(match value {
        "pppp" => DynamicSign::Level(Pppp),
        "ppp" => DynamicSign::Level(Ppp),
        "pp" => DynamicSign::Level(Pp),
        "p" => DynamicSign::Level(P),
        "mp" => DynamicSign::Level(Mp),
        "mf" => DynamicSign::Level(Mf),
        "f" => DynamicSign::Level(F),
        "ff" => DynamicSign::Level(Ff),
        "fff" => DynamicSign::Level(Fff),
        "ffff" => DynamicSign::Level(Ffff),
        "sf" => DynamicSign::Sforzato,
        "sfz" => DynamicSign::Sforzando,
        "fp" => DynamicSign::FortePiano,
        _ => return None,
    })
}
pub(crate) fn symbol_text(value: DynamicSign) -> &'static str {
    use DynamicLevel::*;
    match value {
        DynamicSign::Level(level) => match level {
            Pppp => "pppp",
            Ppp => "ppp",
            Pp => "pp",
            P => "p",
            Mp => "mp",
            Mf => "mf",
            F => "f",
            Ff => "ff",
            Fff => "fff",
            Ffff => "ffff",
        },
        DynamicSign::Sforzato => "sf",
        DynamicSign::Sforzando => "sfz",
        DynamicSign::FortePiano => "fp",
    }
}
pub(crate) fn scope(target: &str, score: &ScoreDocument) -> Option<ScorePartScope> {
    if score.parts.iter().any(|p| p.id.0 == target) {
        return Some(ScorePartScope {
            part: PartId(target.into()),
            staff: None,
        });
    }
    let (part, staff) = target.rsplit_once(".s")?;
    Some(ScorePartScope {
        part: PartId(part.into()),
        staff: Some(staff.parse().ok()?),
    })
}
pub(crate) fn target(scope: &ScorePartScope, part: &str) -> String {
    scope
        .staff
        .map(|s| format!("{part}.s{s}"))
        .unwrap_or_else(|| part.into())
}
fn issue(r: &mut MappingReport, f: Format, feature: &str, path: &str, message: &str) {
    r.push(f, MappingEffect::Unsupported, feature, path, message);
}
pub(crate) fn read_mark(
    mark: &Mark,
    at: &Rational,
    score: &mut ScoreDocument,
    r: &mut MappingReport,
) {
    for key in mark.attributes.keys() {
        if !["value", "sound", "placement"].contains(&key.as_str()) {
            issue(
                r,
                Format::Sfs,
                "dynamics.attribute",
                &mark.target,
                "Unmapped dynamic attributes are not discarded",
            );
        }
    }
    let Some(scope) = scope(&mark.target, score) else {
        issue(
            r,
            Format::Sfs,
            "dynamics.scope",
            &mark.target,
            "Only explicit part/staff display dynamics are mapped",
        );
        return;
    };
    let sign = symbol(mark.attributes["value"].as_str().unwrap()).unwrap();
    let sound = mark
        .attributes
        .get("sound")
        .and_then(Scalar::as_bool)
        .unwrap_or(true);
    let placement = match mark.attributes.get("placement").and_then(Scalar::as_str) {
        Some("above") => Some(Placement::Above),
        Some("below") => Some(Placement::Below),
        _ => None,
    };
    score.dynamics.push(ScoreDynamic {
        at: at.clone(),
        scope,
        sign,
        placement,
        sound,
    });
}
pub(crate) fn read_state(
    state: &State,
    at: &Rational,
    score: &mut ScoreDocument,
    r: &mut MappingReport,
) {
    let Some(scope) = scope(&state.target, score) else {
        issue(
            r,
            Format::Sfs,
            "sound_dynamics.scope",
            &state.target,
            "Only part/staff numeric sound scopes are mapped",
        );
        return;
    };
    let percent = state.attributes["sound_dynamics"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    score.sound_dynamics.push(ScoreSoundDynamics {
        at: at.clone(),
        scope,
        percent,
    });
}
pub(crate) fn write_mark(
    d: &ScoreDynamic,
    part: &str,
    frames: &mut BTreeMap<Rational, Vec<Record>>,
) {
    let mut attributes = Attributes::from([
        ("value".into(), Scalar::String(symbol_text(d.sign).into())),
        ("sound".into(), Scalar::Boolean(d.sound)),
    ]);
    if let Some(place) = d.placement {
        attributes.insert(
            "placement".into(),
            Scalar::String(
                match place {
                    Placement::Above => "above",
                    Placement::Below => "below",
                }
                .into(),
            ),
        );
    }
    frames
        .entry(d.at.clone())
        .or_default()
        .push(Record::Mark(Mark {
            target: target(&d.scope, part),
            kind: "dynamic".into(),
            attributes,
            source: SourceRange::default(),
        }));
}
pub(crate) fn write_state(
    d: &ScoreSoundDynamics,
    part: &str,
    frames: &mut BTreeMap<Rational, Vec<Record>>,
) {
    frames
        .entry(d.at.clone())
        .or_default()
        .push(Record::State(State {
            target: target(&d.scope, part),
            attributes: [(
                "sound_dynamics".into(),
                Scalar::String(d.percent.to_string()),
            )]
            .into(),
            source: SourceRange::default(),
        }));
}
pub(crate) fn normalize(score: &mut ScoreDocument, r: &mut MappingReport, format: Format) {
    score.sound_dynamics.sort_by(|a, b| {
        a.scope
            .part
            .cmp(&b.scope.part)
            .then(a.scope.staff.cmp(&b.scope.staff))
            .then(a.at.cmp(&b.at))
    });
    for pair in score.sound_dynamics.windows(2) {
        if pair[0].scope == pair[1].scope && pair[0].at == pair[1].at {
            let same = pair[0].percent == pair[1].percent;
            r.push(format,if same {MappingEffect::Normalized}else{MappingEffect::Invalid},if same {"sound_dynamics.duplicate"}else{"sound_dynamics.conflict"},target(&pair[1].scope,&pair[1].scope.part.0),"Simultaneous numeric dynamics in one scope must agree exactly; no source-order precedence is guessed");
        }
    }
    score.sound_dynamics.dedup();
    score.dynamics.sort_by(|a, b| {
        a.scope
            .part
            .cmp(&b.scope.part)
            .then(a.scope.staff.cmp(&b.scope.staff))
            .then(a.at.cmp(&b.at))
    });
}
pub(crate) fn validate(score: &ScoreDocument, r: &mut MappingReport) {
    let parts: BTreeMap<_, _> = score
        .parts
        .iter()
        .map(|p| {
            (
                &p.id,
                (
                    p.staves,
                    p.measures
                        .last()
                        .map(|m| &m.start + &m.length)
                        .unwrap_or_default(),
                ),
            )
        })
        .collect();
    let valid = |scope: &ScorePartScope, at: &Rational| {
        parts.get(&scope.part).is_some_and(|(staves, end)| {
            !at.is_negative() && at < end && scope.staff.is_none_or(|s| s > 0 && s <= *staves)
        })
    };
    let mut seen = BTreeMap::new();
    for d in &score.sound_dynamics {
        if !valid(&d.scope, &d.at) || d.percent.is_negative() {
            r.push(Format::ScoreModel,MappingEffect::Invalid,"sound_dynamics.value",d.at.to_string(),"Numeric dynamics need a valid part/staff, onset inside score, and nonnegative exact percent");
        }
        if seen
            .insert((&d.scope.part, d.scope.staff, &d.at), &d.percent)
            .is_some()
        {
            r.push(
                Format::ScoreModel,
                MappingEffect::Invalid,
                "sound_dynamics.conflict",
                d.at.to_string(),
                "One exact numeric sound assignment per scope/position is required",
            );
        }
    }
    for d in &score.dynamics {
        if !valid(&d.scope, &d.at) {
            r.push(
                Format::ScoreModel,
                MappingEffect::Invalid,
                "dynamics.value",
                d.at.to_string(),
                "Printed dynamics require valid part/staff and onset inside score",
            );
        }
    }
}
