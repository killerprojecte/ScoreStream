# Explicit continuation for an authored open final ending

The strict `SfsAdapter`, `from_document`, and `to_document` defaults are unchanged.
An unclosed final ending is still rejected without a caller choice. New
`SfsOptions`/`read_with_options`/`write_with_options` (and AST equivalents) expose
the existing core `UnclosedFinalEndingPolicy::ContinueToMovementEnd`. They use the
same route validator, including its final-navigation-only, nonoverlap, valid
first-ending and pass constraints. No marker, endpoint, source count or note is
invented. SFS retains the original open `start` marker, not an inferred ending.

CLI conversion can opt in with:

```
--unclosed-final-ending continue-to-movement-end
```

The flag requires an SFS input or output and must be supplied again when converting
an open-ending SFS back to MusicXML. The choice is not persisted as authored score
metadata, not a global preference, and not authorization for performed audio.
Strict playback remains unchanged. A successful application reports
`navigation.unclosed_final_ending_continues` as `policy_applied`. Merely passing
the option on a closed route does not invent a continuation decision.

The pinned corpus harness adds `--movement-iv-continue-final-ending`, scoped only
to hash-verified movement IV and its export transcript check. It never applies
the choice to other movements. This composes with exact irregular-measure
contracts rather than replacing their independent scope.

Untouched IV P1 has forward repeat at ordinal/label2, first ending start85,
first ending stop and backward repeat86, and second ending start87 with no
endpoint through the final measure446. This is its last navigation structure.
The source remains open after roundtrip. Full-source XML/MXL and note-event
transcript verification is recorded separately; playback/render completeness is
not implied by successful conversion.
