//! Exact authored state only. No generic instrument is relabeled as pizzicato.
use super::*;
use scorestream_codec::words::PizzicatoMode;
pub(super) struct State<'a>(BTreeMap<&'a PartId, Vec<&'a Direction>>);
impl<'a> State<'a> {
    pub fn new(score: &'a ResolvedScore) -> Result<Self, String> {
        let mut parts: BTreeMap<&PartId, Vec<&Direction>> = BTreeMap::new();
        for d in &score.directions {
            if let DirectionKind::Pizzicato { .. } = d.kind {
                let Scope::Part(part) = &d.scope else {
                    return Err("pizzicato.scope: expected part-wide sound state".into());
                };
                parts.entry(part).or_default().push(d);
            }
        }
        for values in parts.values_mut() {
            values.sort_by(|a, b| a.at.cmp(&b.at));
            if values
                .windows(2)
                .any(|pair| pair[0].at == pair[1].at && pair[0].kind != pair[1].kind)
            {
                return Err(
                    "pizzicato.conflict: conflicting sound states at the same exact part position"
                        .into(),
                );
            }
        }
        Ok(Self(parts))
    }
    pub fn active(&self, event: &ResolvedEvent, at: &Rational) -> bool {
        let Some(values) = self.0.get(&event.part) else {
            return false;
        };
        let i = values.partition_point(|d| d.at <= *at);
        i > 0
            && values[i - 1].end.as_ref().is_none_or(|end| at < end)
            && matches!(
                values[i - 1].kind,
                DirectionKind::Pizzicato {
                    mode: Some(PizzicatoMode::Pizzicato)
                }
            )
    }
    pub fn check(&self, event: &ResolvedEvent, at: &Rational) -> Result<(), String> {
        if self.active(event, at) {
            return Err(format!(
                "pizzicato.not_realized: part {} note {} attacks under authored pizzicato=yes; select a supported pizzicato sound route before playback",
                event.part.0, event.id.0
            ));
        }
        Ok(())
    }
}
