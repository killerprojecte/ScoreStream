use crate::registry::{required_relation_fields, validate_attributes};
use crate::*;
use std::collections::{BTreeMap, BTreeSet};

/// Validate the normalized AST, including ASTs obtained directly from JSON.
/// The caller must reject errors before committing a candidate document.
pub fn validate(doc: &Document, context: Option<&FragmentContext>) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if doc.format != "ScoreStream" || doc.format_version != "1.1" || doc.time_unit != "quarter" {
        out.push(Diagnostic::error(
            "format_unsupported",
            "Only ScoreStream 1.1 quarter-note time is supported",
            doc.source,
        ));
    }
    if doc.end.is_negative() {
        out.push(Diagnostic::error(
            "time_negative",
            "Document end must be nonnegative",
            doc.source,
        ));
    }
    let mut requirements = BTreeSet::new();
    for requirement in &doc.required_extensions {
        if !requirements.insert(requirement) {
            out.push(Diagnostic::error(
                "extension_duplicate",
                "Required extensions must be distinct",
                doc.source,
            ));
        }
        if requirement != crate::navigation_extension::EXTENSION
            && requirement != crate::fermata_extension::EXTENSION
            && requirement != crate::grace_extension::EXTENSION
            && requirement != crate::instrument_extension::EXTENSION
            && requirement != crate::beam_extension::EXTENSION
            && requirement != crate::slur_extension::EXTENSION
            && requirement != crate::articulation_extension::EXTENSION
            && requirement != crate::notation_extension::EXTENSION
            && requirement != crate::provenance::EXTENSION
            && requirement != crate::credits::EXTENSION
            && requirement != crate::defaults::EXTENSION
            && requirement != crate::geometry::EXTENSION
            && requirement != crate::print_layout::EXTENSION
            && requirement != crate::wedges::EXTENSION
            && requirement != crate::dashes::EXTENSION
            && requirement != crate::words::EXTENSION
            && requirement != crate::part_labels::EXTENSION
            && requirement != crate::words::PIZZICATO_EXTENSION
            && requirement != crate::metronome_display::EXTENSION
            && requirement != crate::part_groups::EXTENSION
            && requirement != crate::lyrics::EXTENSION
            && requirement != crate::tuplets::EXTENSION
            && requirement != crate::tuplets::ORNAMENT_EXTENSION
        {
            out.push(Diagnostic::error(
                "extension_unsupported",
                format!("Required extension is not implemented: {requirement}"),
                doc.source,
            ));
        }
    }
    out.extend(doc.metadata.validate(doc.source));
    let mut parts: BTreeMap<&str, u32> = BTreeMap::new();
    let mut voices: BTreeMap<&str, (&str, u32)> = BTreeMap::new();
    let mut aliases = BTreeSet::new();
    aliases.insert("score");
    for part in &doc.parts {
        if !is_identifier(&part.id) || !aliases.insert(&part.id) {
            out.push(Diagnostic::error(
                "alias_duplicate",
                format!("Invalid, reserved, or repeated part alias {}", part.id),
                part.source,
            ));
        }
        if part.staves == 0 {
            out.push(Diagnostic::error(
                "staff_range",
                "Part must contain at least one staff",
                part.source,
            ));
        }
        if part.display_name.as_ref().is_some_and(|name| name.chars().any(|c| {
            !matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
        })) {
            out.push(Diagnostic::error(
                "part_display_name",
                "Part display name must be plain text without XML-forbidden characters",
                part.source,
            ));
        }
        if part.instrument.is_empty()
            || part
                .instrument
                .chars()
                .any(|c| c.is_whitespace() || c.is_control() || c == '"')
        {
            out.push(Diagnostic::error(
                "field_value",
                "Instrument must be a nonempty atom",
                part.source,
            ));
        }
        parts.insert(&part.id, part.staves);
    }
    for voice in &doc.voices {
        if !is_identifier(&voice.id) || !aliases.insert(&voice.id) {
            out.push(Diagnostic::error(
                "alias_duplicate",
                format!("Invalid, reserved, or repeated voice alias {}", voice.id),
                voice.source,
            ));
        }
        check_staff(
            &parts,
            &voice.part,
            voice.staff as i64,
            voice.source,
            &mut out,
        );
        voices.insert(&voice.id, (&voice.part, voice.staff));
    }
    if doc.document_kind == DocumentKind::Fragment {
        if !doc.parts.is_empty()
            || !doc.voices.is_empty()
            || !doc.metadata.is_empty()
            || !doc.required_extensions.is_empty()
        {
            out.push(Diagnostic::error(
                "fragment_global_state",
                "Fragments cannot contain declarations or metadata",
                doc.source,
            ));
        }
        match &doc.span {
            Some(span) if span.is_positive() && span == &doc.end => (),
            _ => out.push(Diagnostic::error(
                "fragment_span",
                "Fragment end must equal its positive span",
                doc.source,
            )),
        }
        if let Some(ctx) = context {
            for binding in &ctx.bindings {
                if !is_identifier(&binding.alias)
                    || binding.alias == "score"
                    || !is_identifier(&binding.part)
                    || binding.part == "score"
                    || binding.alias == binding.part
                    || voices.contains_key(binding.alias.as_str())
                    || parts.contains_key(binding.alias.as_str())
                    || voices.contains_key(binding.part.as_str())
                {
                    out.push(Diagnostic::error(
                        "alias_duplicate",
                        "Invalid or duplicate fragment binding",
                        doc.source,
                    ));
                }
                if parts
                    .get(binding.part.as_str())
                    .is_some_and(|n| *n != binding.staves)
                {
                    out.push(Diagnostic::error(
                        "binding_context",
                        "Bindings disagree on part staff count",
                        doc.source,
                    ));
                }
                parts.insert(&binding.part, binding.staves);
                voices.insert(&binding.alias, (&binding.part, binding.staff));
                check_staff(
                    &parts,
                    &binding.part,
                    binding.staff as i64,
                    doc.source,
                    &mut out,
                );
            }
        } else {
            out.push(Diagnostic::error(
                "fragment_context_required",
                "Provide explicit voice bindings before validating a replacement fragment",
                doc.source,
            ));
        }
    } else if doc.span.is_some() {
        out.push(Diagnostic::error(
            "fragment_span",
            "A complete score cannot carry fragment span",
            doc.source,
        ));
    }
    let mut events: BTreeMap<&str, (&Rational, &Event)> = BTreeMap::new();
    let mut chords: BTreeMap<&str, (&Rational, &Event)> = BTreeMap::new();
    let mut ids = BTreeSet::new();
    let mut mark_ids = BTreeSet::new();
    let mut has_geometry = false;
    let mut has_wedges = false;
    let mut has_dashes = false;
    let mut has_words = false;
    let mut has_part_labels = false;
    let mut has_metronome_display=false;
    let mut has_credits = false;
    let mut has_defaults = false;
    let mut has_provenance = false;
    let mut has_part_groups = false;
    let mut has_print = false;
    let mut has_lyrics = false;
    let mut has_tuplets = false;
    let mut dynamic_qualifier_source = None;
    let mut measures = Vec::new();
    let mut previous: Option<&Rational> = None;
    for frame in &doc.frames {
        if frame.time.is_negative() {
            out.push(Diagnostic::error(
                "time_negative",
                "Frame time must be nonnegative",
                frame.source,
            ));
        }
        if previous.is_some_and(|t| t >= &frame.time) {
            out.push(Diagnostic::error(
                "frame_order",
                "Frame times must strictly increase",
                frame.source,
            ));
        }
        previous = Some(&frame.time);
        if frame.time > doc.end {
            out.push(Diagnostic::error(
                "end_truncates_frame",
                "Frame occurs after document end",
                frame.source,
            ));
        }
        let mut assignments = BTreeMap::new();
        let mut meter_displays = BTreeMap::new();
        for record in &frame.records {
            match record {
                Record::Event(e) => {
                    validate_attributes("event", "", &e.attributes, e.source, &mut out);
                    if let Some(value) = e.attributes.get("attack_dynamics") {
                        if !value.as_str().and_then(|v| v.parse::<Rational>().ok()).is_some_and(|v| !v.is_negative() && crate::geometry::decimal(&v)) || !e.pitch.is_note() {
                            out.push(Diagnostic::error("note_dynamics.value", "Attack dynamics requires a note owner and exact nonnegative finite decimal percentage", e.source));
                        }
                    }
                    let valid_pitch = match &e.pitch {
                        Pitch::Pitched { step, .. } => ('A'..='G').contains(step),
                        Pitch::Unpitched(id) => is_identifier(id),
                        _ => true,
                    };
                    if !valid_pitch {
                        out.push(Diagnostic::error(
                            "pitch_invalid",
                            "Invalid written pitch",
                            e.source,
                        ));
                    }

                    if let Some(id) = e.id() {
                        if events.insert(id, (&frame.time, e)).is_some() {
                            out.push(Diagnostic::error(
                                "id_duplicate",
                                format!("Duplicate event ID {id}"),
                                e.source,
                            ));
                        }
                    }
                    match voices.get(e.voice.as_str()) {
                        Some((part, _)) => {
                            if let Some(staff) = e.attributes.get("staff").and_then(Scalar::as_i64)
                            {
                                check_staff(&parts, part, staff, e.source, &mut out);
                            }
                        }
                        None => out.push(Diagnostic::error(
                            "voice_undeclared",
                            format!("Voice {} is not declared or bound", e.voice),
                            e.source,
                        )),
                    }
                    if let Some(d) = e.duration.measured() {
                        if !d.is_positive() {
                            out.push(Diagnostic::error(
                                "duration_not_positive",
                                "Measured duration must be positive",
                                e.source,
                            ));
                        }
                        if &frame.time + d > doc.end {
                            out.push(Diagnostic::error(
                                "end_truncates_event",
                                "Event tail exceeds document end",
                                e.source,
                            ));
                        }
                        if doc.document_kind == DocumentKind::Fragment
                            && doc
                                .span
                                .as_ref()
                                .is_some_and(|s| &frame.time >= s || &frame.time + d > *s)
                        {
                            out.push(Diagnostic::error(
                                "fragment_span",
                                "Event lies outside fragment interval",
                                e.source,
                            ));
                        }
                    }
                    if let Some(chord) = e.chord() {
                        if !e.pitch.is_note() {
                            out.push(Diagnostic::error(
                                "chord_kind",
                                "Chord cannot contain a rest or spacer",
                                e.source,
                            ));
                        }
                        if let Some((time, head)) = chords.get(chord) {
                            if *time != &frame.time {
                                out.push(Diagnostic::error(
                                    "chord_onset",
                                    "Chord members must share an onset",
                                    e.source,
                                ));
                            }
                            if head.voice != e.voice {
                                out.push(Diagnostic::error(
                                    "chord_voice",
                                    "Chord members must share a voice",
                                    e.source,
                                ));
                            }
                            if head.is_grace() != e.is_grace()
                                || (e.is_grace()
                                    && (head.attributes.get("anchor")
                                        != e.attributes.get("anchor")
                                        || head.attributes.get("order")
                                            != e.attributes.get("order")))
                            {
                                out.push(Diagnostic::error(
                                    "chord_kind",
                                    "Grace chord members must share anchor and order",
                                    e.source,
                                ));
                            }
                        } else {
                            chords.insert(chord, (&frame.time, e));
                        }
                    }
                    validate_rhythm(e, &mut out);
                }
                Record::State(s) => {
                    validate_attributes("state", "", &s.attributes, s.source, &mut out);
                    if doc.document_kind == DocumentKind::Fragment {
                        out.push(Diagnostic::error(
                            "fragment_global_state",
                            "Fragments cannot mutate state",
                            s.source,
                        ));
                    }
                    let scope = scope_kind(&s.target, &parts, &voices);
                    if scope.is_none() {
                        out.push(Diagnostic::error(
                            "scope_unresolved",
                            format!("Unknown scope {}", s.target),
                            s.source,
                        ));
                    }
                    for (key, value) in &s.attributes {
                        let allowed = match key.as_str() {
                            "tempo" => scope == Some("score"),
                            "sound_dynamics" => matches!(scope, Some("part" | "staff")),
                            "meter" | "meter_symbol" | "key" | "clef" => scope == Some("staff"),
                            "staff" => scope == Some("voice"),
                            "instrument" => scope == Some("part"),
                            "transpose_chromatic" | "transpose_diatonic" | "transpose_octaves" => {
                                matches!(scope, Some("staff" | "part"))
                            }
                            _ => true,
                        };
                        if !allowed {
                            out.push(Diagnostic::error(
                                "state_scope",
                                format!("{key} is not valid at {} scope", s.target),
                                s.source,
                            ));
                        }
                        if assignments
                            .insert((&s.target, key), value)
                            .is_some_and(|old| old != value)
                        {
                            out.push(Diagnostic::error(
                                "state_conflict",
                                format!("Conflicting {key} assignments at {}", s.target),
                                s.source,
                            ));
                        }
                    }
                    if s.attributes.contains_key("meter") {
                        let symbol=s.attributes.get("meter_symbol");
                        if meter_displays.insert(&s.target,symbol).is_some_and(|old|old!=symbol) {out.push(Diagnostic::error("meter_symbol_conflict","Conflicting authored symbol presence at one staff/onset",s.source));}
                    }
                    if let Some(symbol)=s.attributes.get("meter_symbol") {
                        let valid=symbol.as_str().and_then(crate::score::TimeSymbol::from_name).is_some_and(|symbol| {
                            s.attributes.get("meter").and_then(Scalar::as_str)==Some(match symbol {crate::score::TimeSymbol::Common=>"4/4",crate::score::TimeSymbol::Cut=>"2/2"})
                        });
                        if !valid {out.push(Diagnostic::error("meter_symbol","Display symbol needs its matching numeric meter in the same staff state record",s.source));}
                    }
                    if let Some(staff) = s.attributes.get("staff").and_then(Scalar::as_i64) {
                        if let Some((part, _)) = voices.get(s.target.as_str()) {
                            check_staff(&parts, part, staff, s.source, &mut out);
                        }
                    }
                    let tuple = [
                        "transpose_chromatic",
                        "transpose_diatonic",
                        "transpose_octaves",
                    ];
                    let count = tuple
                        .iter()
                        .filter(|k| s.attributes.contains_key(**k))
                        .count();
                    if count != 0 && count != 3 {
                        out.push(Diagnostic::error(
                            "transposition_tuple",
                            "Assign chromatic, diatonic, and octave transposition together",
                            s.source,
                        ));
                    }
                }
                Record::Measure(m) => {
                    validate_attributes("measure", "", &m.attributes, m.source, &mut out);
                    if !is_identifier(&m.id) || !ids.insert(&m.id) {
                        out.push(Diagnostic::error(
                            "id_duplicate",
                            "Invalid or duplicate measure ID",
                            m.source,
                        ));
                    }
                    if doc.document_kind == DocumentKind::Fragment {
                        out.push(Diagnostic::error(
                            "fragment_global_state",
                            "Fragments cannot mutate measures",
                            m.source,
                        ));
                    }
                    if !matches!(
                        scope_kind(&m.target, &parts, &voices),
                        Some("part" | "staff")
                    ) {
                        out.push(Diagnostic::error(
                            "measure_scope",
                            "Measure target must be a declared part or staff",
                            m.source,
                        ));
                    }
                    if !m.length.is_positive() {
                        out.push(Diagnostic::error(
                            "duration_not_positive",
                            "Measure length must be positive",
                            m.source,
                        ));
                    }
                    if &frame.time + &m.length > doc.end {
                        out.push(Diagnostic::error(
                            "end_truncates_measure",
                            "Measure exceeds document end",
                            m.source,
                        ));
                    }
                    measures.push((&frame.time, m));
                }
                Record::Mark(m) => {
                    validate_attributes("mark", &m.kind, &m.attributes, m.source, &mut out);
                    if m.kind == "dynamic" {
                        let text = m.attributes.get("qualifier_text");
                        let position = m.attributes.get("qualifier_position");
                        if text.is_some() || position.is_some() {
                            dynamic_qualifier_source.get_or_insert(m.source);
                            if !text.and_then(Scalar::as_str).is_some_and(|s| !s.trim().is_empty())
                                || !position.and_then(Scalar::as_str).is_some_and(|s| crate::score::DynamicQualifierPosition::from_name(s).is_some()) {
                                out.push(Diagnostic::error("dynamics.qualifier", "A dynamic qualifier requires nonblank exact text and an explicit before/after position", m.source));
                            }
                        }
                        for key in ["default_x", "default_y", "relative_x", "relative_y"] {
                            if m.attributes.get(key).is_some_and(|value| {
                                !value.as_str()
                                    .and_then(|s| s.parse::<Rational>().ok())
                                    .is_some_and(|v| crate::geometry::decimal(&v))
                            }) {
                                out.push(Diagnostic::error(
                                    "dynamics.geometry",
                                    "Dynamic coordinates must be exact finite decimals",
                                    m.source,
                                ));
                            }
                        }
                    }
                    has_geometry |= crate::geometry::is_kind(&m.kind);
                    has_wedges |= crate::wedges::is_kind(&m.kind);
                    has_dashes |= crate::dashes::is_kind(&m.kind);
                    has_words |= crate::words::is_kind(&m.kind);
                    has_part_labels |= crate::part_labels::is_kind(&m.kind);
                    has_part_groups |= crate::part_groups::is_kind(&m.kind);
                    has_defaults |= crate::defaults::is_kind(&m.kind);
                    has_metronome_display |= crate::metronome_display::is_kind(&m.kind);
                    has_credits |= crate::credits::is_kind(&m.kind);
                    has_provenance |= crate::provenance::is_kind(&m.kind);
                    has_print |= crate::print_layout::is_kind(&m.kind);
                    has_lyrics |= crate::lyrics::is_kind(&m.kind);
                    has_tuplets |= crate::tuplets::is_kind(&m.kind);
                    if let Some(id) = m.attributes.get("id").and_then(Scalar::as_str) {
                        if !mark_ids.insert(id) {
                            out.push(Diagnostic::error(
                                "id_duplicate",
                                "Duplicate mark ID",
                                m.source,
                            ));
                        }
                    }
                }
                Record::Relation(r) => {
                    validate_attributes("relation", &r.kind, &r.attributes, r.source, &mut out)
                }
            }
        }
    }
    if let Some(source) = dynamic_qualifier_source {
        out.push(Diagnostic::warning("dynamics.qualifier_unrealized", "Ordered dynamic qualifier text is preserved but not rendered in the current reflow preview. Explicit sound=false keeps numeric playback unchanged without realizing qualifier expression; sounding qualifiers require an explicit interpretation", source));
    }
    if has_wedges || doc.required_extensions.iter().any(|e|e==crate::wedges::EXTENSION) {
        match crate::wedges::decode(doc) {
            Err(errors)=>out.extend(errors),
            Ok(endpoints)=>{
                out.push(Diagnostic::warning("wedge.authored_only", "Authored wedge endpoints are retained; current preview and performance do not realize wedge shape/expression. No target loudness or curve is inferred",doc.source));
                let count=crate::wedges::unresolved_pairs(&endpoints);
                if count>0 {out.push(Diagnostic::warning("wedge.pairing_unresolved",format!("{count} unmatched/ambiguous authored wedge endpoints; no pairing repair is inferred"),doc.source));}
            }
        }
    }
    if has_dashes || doc.required_extensions.iter().any(|e|e==crate::dashes::EXTENSION) {
        match crate::dashes::decode(doc) {
            Err(errors)=>out.extend(errors),
            Ok(endpoints)=>{
                out.push(Diagnostic::warning("dashes.authored_only", "Authored dashes endpoints are retained; current preview and performance do not realize words/dashes shape or expression. No gain or tempo curve is inferred",doc.source));
                let count=crate::dashes::unresolved_pairs(&endpoints);
                if count>0 {out.push(Diagnostic::warning("dashes.pairing_unresolved",format!("{count} unmatched/ambiguous authored dashes endpoints; no pairing repair is inferred"),doc.source));}
            }
        }
    }
    if has_words || doc.required_extensions.iter().any(|e|e==crate::words::EXTENSION || e==crate::words::PIZZICATO_EXTENSION) {
        match crate::words::decode(doc) {
            Err(errors)=>out.extend(errors),
            Ok(_)=>{
                if doc.required_extensions.iter().any(|e|e==crate::words::PIZZICATO_EXTENSION) { out.push(Diagnostic::warning("pizzicato.not_realized", "Authored yes/no sound state retained independently of words. No supported pizzicato route is selected; active pizzicato at a sounding attack must be resolved before playback",doc.source)); }
                out.push(Diagnostic::warning("words.authored_only", "Authored score words are retained; current preview omits them and performance does not interpret their instructions. No gain, tempo, timbre or navigation behavior is inferred",doc.source));
            }
        }
    }
    if has_part_labels || doc.required_extensions.iter().any(|e|e==crate::part_labels::EXTENSION) {
        match crate::part_labels::decode(doc) {
            Err(errors)=>out.extend(errors),
            Ok(_)=>out.push(Diagnostic::warning("part_labels.authored_only","Authored part abbreviations and name-display details are retained; current preview does not realize these extra labels. Legacy part names and instrument routing are unchanged",doc.source)),
        }
    }
    if has_provenance || doc.required_extensions.iter().any(|e| e == crate::provenance::EXTENSION) {
        if let Err(errors) = crate::provenance::decode(doc) { out.extend(errors); }
    }
    if has_metronome_display || doc.required_extensions.iter().any(|e|e==crate::metronome_display::EXTENSION) {
        match crate::metronome_display::decode(doc) {
            Err(errors)=>out.extend(errors),
            Ok(_)=>out.push(Diagnostic::warning("metronome_display.authored_only","Metronome group text, parentheses and geometry are preserved but not realized by current preview. Existing numeric printed mark and explicit sounding tempo remain independent",doc.source)),
        }
    }
    crate::navigation_extension::validate(doc, &mut out);
    crate::fermata_extension::validate(doc, &mut out);
    crate::grace_extension::validate(doc, &mut out);
    crate::instrument_extension::validate(doc, &mut out);
    crate::beam_extension::validate(doc, &mut out);
    crate::slur_extension::validate(doc, &mut out);
    crate::articulation_extension::validate(doc, &mut out);
    crate::notation_extension::validate(doc, &mut out);
    if has_part_groups || doc.required_extensions.iter().any(|e|e==crate::part_groups::EXTENSION) {if let Err(errors)=crate::part_groups::decode(doc){out.extend(errors);}}
    if has_defaults || doc.required_extensions.iter().any(|e|e==crate::defaults::EXTENSION) {if let Err(errors)=crate::defaults::decode(doc){out.extend(errors);}}
    if has_credits || doc.required_extensions.iter().any(|e| e == crate::credits::EXTENSION) {
        if let Err(errors) = crate::credits::decode(doc) {
            out.extend(errors);
        }
    }
    if has_geometry
        || doc
            .required_extensions
            .iter()
            .any(|e| e == crate::geometry::EXTENSION)
    {
        crate::geometry::validate(doc, &mut out);
    }
    if has_print
        || doc
            .required_extensions
            .iter()
            .any(|e| e == crate::print_layout::EXTENSION)
    {
        crate::print_layout::validate(doc, &mut out);
    }
    if has_lyrics
        || doc
            .required_extensions
            .iter()
            .any(|e| e == crate::lyrics::EXTENSION)
    {
        crate::lyrics::validate(doc, &mut out);
    }
    if has_tuplets
        || doc
            .required_extensions
            .iter()
            .any(|e| e == crate::tuplets::EXTENSION || e == crate::tuplets::ORNAMENT_EXTENSION)
    {
        crate::tuplets::validate(doc, &mut out);
    }
    validate_measures(&measures, &parts, &mut out);
    validate_measure_rests(doc, &measures, &voices, &mut out);
    validate_references(doc, &events, &parts, &voices, &mut out);
    if doc.document_kind == DocumentKind::Fragment {
        if let Some(span) = &doc.span {
            for alias in voices.keys() {
                validate_coverage(doc, alias, span, &mut out);
            }
        }
    }
    out
}
fn check_staff(
    parts: &BTreeMap<&str, u32>,
    part: &str,
    staff: i64,
    source: SourceRange,
    out: &mut Vec<Diagnostic>,
) {
    if !parts
        .get(part)
        .is_some_and(|count| staff > 0 && staff <= *count as i64)
    {
        out.push(Diagnostic::error(
            "staff_range",
            format!("Staff {staff} is outside declared part {part}"),
            source,
        ));
    }
}
fn scope_kind(
    target: &str,
    parts: &BTreeMap<&str, u32>,
    voices: &BTreeMap<&str, (&str, u32)>,
) -> Option<&'static str> {
    if target == "score" {
        Some("score")
    } else if parts.contains_key(target) {
        Some("part")
    } else if voices.contains_key(target) {
        Some("voice")
    } else if target.rsplit_once(".s").is_some_and(|(p, s)| {
        s.parse::<u32>().ok().is_some_and(|staff| {
            staff > 0 && parts.get(p).is_some_and(|count| staff <= *count) && s == staff.to_string()
        })
    }) {
        Some("staff")
    } else {
        None
    }
}
fn validate_measures(
    measures: &[(&Rational, &Measure)],
    parts: &BTreeMap<&str, u32>,
    out: &mut Vec<Diagnostic>,
) {
    // Track only staff scopes actually mentioned, never allocate one entry per
    // declared staff. A large valid staff count must not cause an allocation bomb.
    let mut part_ends: BTreeMap<&str, Rational> = BTreeMap::new();
    let mut staff_ends: BTreeMap<&str, BTreeMap<u32, Rational>> = BTreeMap::new();
    for (start, measure) in measures {
        let target = measure.target.as_str();
        let overlaps = if parts.contains_key(target) {
            let conflict = part_ends.get(target).is_some_and(|end| end > *start)
                || staff_ends
                    .get(target)
                    .is_some_and(|ends| ends.values().any(|end| end > *start));
            part_ends.insert(target, *start + &measure.length);
            conflict
        } else if let Some((part, staff)) = target.rsplit_once(".s") {
            if let Ok(staff) = staff.parse::<u32>() {
                let ends = staff_ends.entry(part).or_default();
                let conflict = part_ends.get(part).is_some_and(|end| end > *start)
                    || ends.get(&staff).is_some_and(|end| end > *start);
                ends.insert(staff, *start + &measure.length);
                conflict
            } else {
                false
            }
        } else {
            false
        };
        if overlaps {
            out.push(Diagnostic::error(
                "measure_overlap",
                "Conflicting overlapping measure ownership",
                measure.source,
            ));
        }
    }
}
fn validate_coverage(doc: &Document, alias: &str, span: &Rational, out: &mut Vec<Diagnostic>) {
    let mut end = Rational::zero();
    for (time, event) in doc.events().filter(|(_, e)| e.voice == alias) {
        if let Some(duration) = event.duration.measured() {
            if time > &end {
                out.push(Diagnostic::error(
                    "fragment_gap",
                    format!("Replacement voice {alias} has an uncovered interval"),
                    event.source,
                ));
                return;
            }
            end = end.max(time + duration);
        }
    }
    if &end != span {
        out.push(Diagnostic::error(
            "fragment_gap",
            format!("Replacement voice {alias} does not cover the full fragment span"),
            doc.source,
        ));
    }
}
/// Check authored full-rest identity against explicit actual measure ownership.
/// State records at an onset apply before events, regardless of record ordering.
fn validate_measure_rests(
    doc: &Document,
    measures: &[(&Rational, &Measure)],
    voices: &BTreeMap<&str, (&str, u32)>,
    out: &mut Vec<Diagnostic>,
) {
    if !doc.frames.iter().flat_map(|f| &f.records).any(|record| {
        matches!(record, Record::Event(e) if e.attributes.get("measure_rest").and_then(Scalar::as_bool) == Some(true))
    }) {
        return;
    }
    let index: BTreeMap<_, _> = measures
        .iter()
        .map(|(at, m)| ((m.target.as_str(), *at), *m))
        .collect();
    let mut staff_defaults: BTreeMap<_, _> = voices
        .iter()
        .map(|(id, (_, staff))| (*id, i64::from(*staff)))
        .collect();
    for frame in &doc.frames {
        for record in &frame.records {
            if let Record::State(s) = record {
                if let Some(staff) = s.attributes.get("staff").and_then(Scalar::as_i64) {
                    if let Some(current) = staff_defaults.get_mut(s.target.as_str()) {
                        *current = staff;
                    }
                }
            }
        }
        for record in &frame.records {
            let Record::Event(e) = record else { continue };
            if e.attributes.get("measure_rest").and_then(Scalar::as_bool) != Some(true) {
                continue;
            }
            let owner = voices.get(e.voice.as_str()).and_then(|(part, _)| {
                let staff = e
                    .attributes
                    .get("staff")
                    .and_then(Scalar::as_i64)
                    .or_else(|| staff_defaults.get(e.voice.as_str()).copied())?;
                index.get(&(*part, &frame.time)).copied().or_else(|| {
                    index
                        .get(&(format!("{part}.s{staff}").as_str(), &frame.time))
                        .copied()
                })
            });
            if doc.document_kind == DocumentKind::Fragment
                || !owner.is_some_and(|m| e.duration.measured() == Some(&m.length))
            {
                out.push(Diagnostic::error("rest.measure_extent",
                    "Full-measure rest requires its exact owning explicit measure at this onset and the same actual duration",
                    e.source));
            }
        }
    }
}
fn validate_rhythm(e: &Event, out: &mut Vec<Diagnostic>) {
    if e.attributes.contains_key("measure_rest") && (e.pitch != Pitch::Rest || e.is_grace()) {
        out.push(Diagnostic::error(
            "rest.measure_kind",
            "Measure-rest identity requires a measured rest",
            e.source,
        ));
    }
    let kind = e.attributes.get("kind").and_then(Scalar::as_str);
    if e.is_grace() {
        if kind != Some("grace") || !e.pitch.is_note() {
            out.push(Diagnostic::error(
                "grace_kind",
                "Grace duration requires a grace note",
                e.source,
            ));
        }
        if !e.attributes.contains_key("anchor") {
            out.push(Diagnostic::error(
                "grace_anchor",
                "Grace note requires a principal anchor",
                e.source,
            ));
        }
        if !e
            .attributes
            .get("order")
            .and_then(Scalar::as_i64)
            .is_some_and(|v| v != 0)
        {
            out.push(Diagnostic::error(
                "grace_order",
                "Grace order must be a nonzero integer",
                e.source,
            ));
        }
    } else if kind == Some("grace")
        || ["anchor", "order", "slash"]
            .iter()
            .any(|key| e.attributes.contains_key(*key))
    {
        out.push(Diagnostic::error(
            "grace_kind",
            "Grace fields require a grace duration",
            e.source,
        ));
    }
    if e.attributes.contains_key("note_size") && !e.attributes.contains_key("w") {
        out.push(Diagnostic::error(
            "note_size.written_type",
            "Graphic note size requires an explicit written type w",
            e.source,
        ));
    }
    if (e.attributes.contains_key("tm_normal_type") || e.attributes.contains_key("tm_normal_dots"))
        && !e.attributes.contains_key("tm")
    {
        out.push(Diagnostic::error(
            "time_modification",
            "Explicit normal notation requires tm",
            e.source,
        ));
    }
    let Some(duration) = e.duration.measured() else {
        return;
    };
    let Some(w) = e.attributes.get("w").and_then(Scalar::as_str) else {
        return;
    };
    if e.attributes.contains_key("tm_normal_type") || e.attributes.contains_key("tm_normal_dots") {
        out.push(Diagnostic::warning("written_normal_notation_preserved", "Explicit normal notation is retained; rhythm consistency inference is not implemented for this spelling", e.source));
        return;
    }
    let base: Option<Rational> = match w {
        "breve" => Some(Rational::from_integer(8)),
        "long" => Some(Rational::from_integer(16)),
        "maxima" => Some(Rational::from_integer(32)),
        _ => w.parse::<i64>().ok().filter(|n| *n > 0).map(|n| {
            let four = Rational::from_integer(4);
            let inverse: Rational = if n == 1 {
                Rational::one()
            } else {
                format!("1/{n}").parse().unwrap()
            };
            &four * &inverse
        }),
    };
    let Some(mut written) = base else {
        return;
    };
    let dots = e
        .attributes
        .get("dots")
        .and_then(Scalar::as_i64)
        .unwrap_or(0);
    if !(0..=4096).contains(&dots) {
        if dots > 4096 {
            out.push(Diagnostic::warning(
                "rhythm_check_limit",
                "Written rhythm check skipped for more than 4096 dots",
                e.source,
            ));
        }
        return;
    }
    let mut added = written.clone();
    let half: Rational = "1/2".parse().unwrap();
    for _ in 0..dots {
        added = &added * &half;
        written = &written + &added;
    }
    if let Some(tm) = e.attributes.get("tm").and_then(Scalar::as_str) {
        if let Some((actual, normal)) = tm.split_once(':') {
            if crate::registry::valid_ratio(tm) {
                // Construct the exact quotient with arbitrary precision, including
                // reducible pairs that are valid written time modifications.
                let a: Rational = actual.parse().unwrap();
                let n: Rational = normal.parse().unwrap();
                written = &written * &(&n / &a);
            } else {
                return;
            }
        }
    }
    if &written != duration {
        out.push(Diagnostic::warning(
            "written_duration_mismatch",
            format!("Written rhythm implies {written}, measured duration remains {duration}"),
            e.source,
        ));
    }
}
fn validate_references<'a>(
    doc: &'a Document,
    events: &BTreeMap<&str, (&'a Rational, &'a Event)>,
    parts: &BTreeMap<&str, u32>,
    voices: &BTreeMap<&str, (&str, u32)>,
    out: &mut Vec<Diagnostic>,
) {
    let mut grace_orders: BTreeMap<(String, i64), Option<&str>> = BTreeMap::new();
    let mut relations: BTreeMap<&str, Vec<(&Rational, &Relation)>> = BTreeMap::new();
    for frame in &doc.frames {
        for record in &frame.records {
            match record {
                Record::Event(e) if e.is_grace() => {
                    if let Some(anchor) = e
                        .attributes
                        .get("anchor")
                        .and_then(Scalar::as_str)
                        .and_then(|s| s.strip_prefix('#'))
                    {
                        match events.get(anchor) {
                            Some((time, principal)) if *time == &frame.time && !principal.is_grace() && principal.pitch.is_note() && principal.voice == e.voice => {
                                if let Some(order) = e.attributes.get("order").and_then(Scalar::as_i64) {
                                    let group = principal.chord().map(|s| format!("chord:{s}")).unwrap_or_else(|| format!("event:{anchor}"));
                                    let key = (group, order);
                                    if let Some(previous_chord) = grace_orders.get(&key) {
                                        if previous_chord.is_none() || *previous_chord != e.chord() { out.push(Diagnostic::error("grace_order_duplicate", "Grace order must be unique per principal attack group", e.source)); }
                                    } else { grace_orders.insert(key, e.chord()); }
                                }
                            },
                            _ => out.push(Diagnostic::error("grace_anchor", "Grace anchor must resolve to a non-grace note in the same frame and voice", e.source)),
                        }
                    }
                }
                Record::Mark(m) => {
                    if m.kind == crate::metronome_display::KIND || m.kind == crate::words::KIND || m.kind == crate::dashes::KIND || m.kind == crate::wedges::KIND || m.kind == crate::geometry::MEASURE || m.kind == crate::print_layout::KIND {
                        continue;
                    }
                    if m.target.starts_with('#') {
                        if let Some((_, e)) = resolve(&m.target, &frame.time, events, m.source, out)
                        {
                            if m.kind == "accidental" {
                                if let Pitch::Pitched { alteration, .. } = &e.pitch {
                                    let expected =
                                        match m.attributes.get("value").and_then(Scalar::as_str) {
                                            Some("natural") => Some("0"),
                                            Some("sharp") => Some("1"),
                                            Some("flat") => Some("-1"),
                                            Some("double-sharp") => Some("2"),
                                            Some("flat-flat") => Some("-2"),
                                            Some("quarter-sharp") => Some("1/2"),
                                            Some("quarter-flat") => Some("-1/2"),
                                            _ => None,
                                        };
                                    if expected.is_some_and(|s| s != alteration.to_string()) {
                                        out.push(Diagnostic::error(
                                            "accidental_pitch_mismatch",
                                            "Printed accidental conflicts with written pitch",
                                            m.source,
                                        ));
                                    }
                                } else {
                                    out.push(Diagnostic::error(
                                        "mark_target",
                                        "Accidental requires a pitched note",
                                        m.source,
                                    ));
                                }
                            }
                        }
                    } else if scope_kind(&m.target, parts, voices).is_none() {
                        out.push(Diagnostic::error(
                            "scope_unresolved",
                            "Mark scope does not resolve",
                            m.source,
                        ));
                    } else if ["accidental", "articulation", "lyric"].contains(&m.kind.as_str()) {
                        out.push(Diagnostic::error(
                            "mark_target",
                            format!("{} requires an event target", m.kind),
                            m.source,
                        ));
                    }
                }
                Record::Relation(r) => {
                    if !is_identifier(&r.id) {
                        out.push(Diagnostic::error(
                            "id_invalid",
                            "Invalid relation identity",
                            r.source,
                        ));
                    }
                    resolve(&r.anchor, &frame.time, events, r.source, out);
                    relations.entry(&r.id).or_default().push((&frame.time, r));
                }
                _ => (),
            }
        }
    }
    for anchors in relations.values_mut() {
        anchors.sort_by(|(ta, a), (tb, b)| {
            ta.cmp(tb)
                .then(phase_order(a.phase).cmp(&phase_order(b.phase)))
        });
        let (_, first) = anchors[0];
        if first.phase != RelationPhase::Start {
            out.push(Diagnostic::error(
                "relation_phase",
                "Relation has an endpoint without a start",
                first.source,
            ));
            continue;
        }
        for field in required_relation_fields(&first.kind) {
            if !first.attributes.contains_key(field) {
                out.push(Diagnostic::error(
                    "field_required",
                    format!("{} start requires {field}", first.kind),
                    first.source,
                ));
            }
        }
        let mut ended = false;
        let mut previous_event = events
            .get(first.anchor.strip_prefix('#').unwrap_or(""))
            .copied();
        for (index, (_, r)) in anchors.iter().enumerate() {
            if r.kind != first.kind {
                out.push(Diagnostic::error(
                    "id_duplicate",
                    "Relation ID cannot be reused for another kind",
                    r.source,
                ));
            }
            if index > 0 && (r.phase == RelationPhase::Start || ended) {
                out.push(Diagnostic::error(
                    "relation_phase",
                    "Relation must have exactly one start and one final end",
                    r.source,
                ));
            }
            if r.phase == RelationPhase::End {
                ended = true;
            }
            if index == 0 {
                continue;
            }
            for (key, value) in &r.attributes {
                if r.kind == "slur"
                    && doc
                        .required_extensions
                        .iter()
                        .any(|e| e == crate::slur_extension::EXTENSION)
                    && crate::slur_extension::is_endpoint_field(key)
                {
                    continue;
                }
                if r.kind == "pedal" && key == "value" {
                    continue;
                }
                let default = if r.kind == "tie" && ["sound", "print"].contains(&key.as_str()) {
                    Some(Scalar::Boolean(true))
                } else {
                    None
                };
                if first.attributes.get(key).or(default.as_ref()) != Some(value) {
                    out.push(Diagnostic::error(
                        "relation_attribute_conflict",
                        format!("Immutable relation attribute {key} differs from start"),
                        r.source,
                    ));
                }
            }
            let current = events
                .get(r.anchor.strip_prefix('#').unwrap_or(""))
                .copied();
            if let (Some((start_time, start)), Some((end_time, end))) = (previous_event, current) {
                if (r.kind == "tuplet" || r.kind == "beam") && start.voice != end.voice {
                    out.push(Diagnostic::error(
                        if r.kind == "tuplet" {
                            "tuplet_voice"
                        } else {
                            "beam_voice"
                        },
                        "Group endpoints must share a voice",
                        r.source,
                    ));
                }
                if r.kind == "tie" {
                    if start.id() == end.id()
                        || !start.pitch.is_note()
                        || !end.pitch.is_note()
                        || start.is_grace()
                        || end.is_grace()
                    {
                        out.push(Diagnostic::error(
                            "tie_anchor",
                            "Tie links distinct measured notes",
                            r.source,
                        ));
                    }
                    if first
                        .attributes
                        .get("sound")
                        .and_then(Scalar::as_bool)
                        .unwrap_or(true)
                    {
                        let pitch_compatible = match (&start.pitch, &end.pitch) {
                            (Pitch::Unpitched(a), Pitch::Unpitched(b)) => a == b,
                            (Pitch::Pitched { .. }, Pitch::Pitched { .. }) => {
                                sounding_pitch(doc, start_time, start, voices)
                                    == sounding_pitch(doc, end_time, end, voices)
                            }
                            _ => false,
                        };
                        if !pitch_compatible {
                            out.push(Diagnostic::error(
                                "tie_pitch",
                                "Sounding tie pitches are incompatible",
                                r.source,
                            ));
                        }
                        if start
                            .duration
                            .measured()
                            .is_none_or(|d| &(start_time + d) != end_time)
                        {
                            out.push(Diagnostic::error(
                                "tie_continuity",
                                "Sounding tie must meet the preceding note tail exactly",
                                r.source,
                            ));
                        }
                        if doc.frames.iter().flat_map(|f| &f.records).any(|record| matches!(record, Record::Relation(o) if o.kind == "octave" && o.attributes.get("sound").and_then(Scalar::as_bool) == Some(true))) { out.push(Diagnostic::error("tie_octave_context_unsupported", "Tie sounding-pitch validation across sounding octave relations is not yet implemented", r.source)); }
                    }
                }
            }
            previous_event = current;
        }
        if !ended {
            out.push(Diagnostic::error(
                "relation_unclosed",
                format!("Relation {} has no end", first.id),
                first.source,
            ));
        }
    }
}
fn phase_order(p: RelationPhase) -> u8 {
    match p {
        RelationPhase::Start => 0,
        RelationPhase::Continue => 1,
        RelationPhase::End => 2,
    }
}
fn resolve<'a>(
    reference: &str,
    time: &Rational,
    events: &BTreeMap<&str, (&'a Rational, &'a Event)>,
    source: SourceRange,
    out: &mut Vec<Diagnostic>,
) -> Option<(&'a Rational, &'a Event)> {
    let found = reference
        .strip_prefix('#')
        .and_then(|id| events.get(id))
        .copied();
    match found {
        None => out.push(Diagnostic::error(
            "reference_unresolved",
            format!("Unresolved event reference {reference}"),
            source,
        )),
        Some((onset, _)) if onset > time => out.push(Diagnostic::error(
            "reference_future_frame",
            "Event attachment occurs before the referenced onset frame",
            source,
        )),
        Some((onset, _)) if onset < time => out.push(Diagnostic::error(
            "reference_frame",
            "Event attachment must be in its onset frame",
            source,
        )),
        _ => (),
    }
    found
}
fn sounding_pitch(
    doc: &Document,
    at: &Rational,
    event: &Event,
    voices: &BTreeMap<&str, (&str, u32)>,
) -> Option<Rational> {
    let pitch = event.pitch.semitones()?;
    let &(part, mut staff) = voices.get(event.voice.as_str())?;
    let mut transpositions: BTreeMap<&str, Rational> = BTreeMap::new();
    for frame in doc.frames.iter().take_while(|f| &f.time <= at) {
        for record in &frame.records {
            if let Record::State(state) = record {
                if state.target == event.voice {
                    if let Some(n) = state
                        .attributes
                        .get("staff")
                        .and_then(Scalar::as_i64)
                        .and_then(|n| u32::try_from(n).ok())
                    {
                        staff = n;
                    }
                }
                if let Some(chromatic) = state
                    .attributes
                    .get("transpose_chromatic")
                    .and_then(Scalar::as_str)
                    .and_then(|v| v.parse::<Rational>().ok())
                {
                    let octaves = Rational::from_integer(
                        state
                            .attributes
                            .get("transpose_octaves")
                            .and_then(Scalar::as_i64)
                            .unwrap_or(0),
                    );
                    transpositions.insert(
                        &state.target,
                        &chromatic + &(&octaves * &Rational::from_integer(12)),
                    );
                }
            }
        }
    }
    if let Some(n) = event
        .attributes
        .get("staff")
        .and_then(Scalar::as_i64)
        .and_then(|n| u32::try_from(n).ok())
    {
        staff = n;
    }
    let key = format!("{part}.s{staff}");
    Some(
        &pitch
            + transpositions
                .get(key.as_str())
                .or_else(|| transpositions.get(part))
                .unwrap_or(&Rational::zero()),
    )
}
