use scorestream_core::{Rational, instrument_metadata::*};

fn sample() -> InstrumentMetadata {
    InstrumentMetadata {
        source_id: "P1-I1".into(),
        name: "B♭ Clarinet".into(),
        abbreviation: None,
        sound: Some("wind.reed.clarinet.bflat".into()),
        midi_device: Some(MidiDevice {
            name: "".into(),
            instrument_ref: Some("P1-I1".into()),
            port: None,
        }),
        midi_instrument: Some(MidiInstrument {
            instrument_ref: "P1-I1".into(),
            channel: Some(12),
            program: Some(72),
            volume: Some("393701/5000".parse().unwrap()),
            pan: Some(Rational::zero()),
        }),
    }
}
#[test]
fn authored_identity_and_assignments_are_exact_independent_and_roundtrip() {
    let value = sample();
    validate(&value).unwrap();
    assert_eq!(value.midi_device.as_ref().unwrap().port, None);
    assert_eq!(
        value
            .midi_instrument
            .as_ref()
            .unwrap()
            .volume
            .as_ref()
            .unwrap()
            .to_string(),
        "393701/5000"
    );
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(
        serde_json::from_str::<InstrumentMetadata>(&json).unwrap(),
        value
    );
    assert_eq!(
        catalog_projection(&value).unwrap(),
        CatalogProjection {
            canonical_id: "clarinet",
            broader: true
        }
    );
}
#[test]
fn source_ids_midi_ranges_and_idrefs_are_validated_without_defaults() {
    for edit in [0, 1, 2, 3, 4, 5, 6] {
        let mut value = sample();
        match edit {
            0 => value.source_id = "bad id".into(),
            1 => value.midi_device.as_mut().unwrap().port = Some(0),
            2 => value.midi_instrument.as_mut().unwrap().channel = Some(17),
            3 => value.midi_instrument.as_mut().unwrap().program = Some(0),
            4 => value.midi_instrument.as_mut().unwrap().instrument_ref = "other".into(),
            5 => value.midi_instrument.as_mut().unwrap().volume = Some(Rational::from_integer(101)),
            _ => value.midi_instrument.as_mut().unwrap().pan = Some(Rational::from_integer(-181)),
        }
        assert!(validate(&value).is_err());
    }
    let mut value = sample();
    value.midi_instrument.as_mut().unwrap().program = Some(128);
    value.midi_device.as_mut().unwrap().instrument_ref = None;
    validate(&value).unwrap();
}
#[test]
fn unknown_sounds_do_not_guess_from_names_or_programs() {
    let mut value = sample();
    value.sound = Some("wind.reed.clarinet.extra".into());
    assert!(catalog_projection(&value).is_none());
    value.sound = None;
    assert!(catalog_projection(&value).is_none());
    value.name = "flute".into();
    assert_eq!(catalog_projection(&value).unwrap().canonical_id, "flute");
    value.name = "Flute".into();
    assert_eq!(catalog_projection(&value).unwrap().canonical_id, "flute");
    value.name = "FLUTE".into();
    assert!(catalog_projection(&value).is_none());
}
