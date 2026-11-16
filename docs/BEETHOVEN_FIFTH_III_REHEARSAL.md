# Third movement: verified first rehearsal export

This is an explicitly selected **attack-only rehearsal approximation**, not faithful expressive rendering. It uses the unchanged, independently roundtrip-verified III SFS (`8e111fdf22f2bf82d891c62028e914172eaa0333ea35cc1678205eb71081c360`), source-bound selected grace/hold timings, explicit practice omissions, and verified TimGM6mb bank 0 (`c5378b62028c920cb11e4803327983fee2f2cdff5dc89c708e39da417e51c854`). No score rewrite or attacca trimming was performed.

- Application plan and native preflight pass: 5,949 audible notes, 141 pizzicato attacks, 248.33333333333334 score seconds.
- Caller note budget is exactly the observed compiled 5,949 notes, not a new product limit.
- One authorized native WAV export succeeds: stereo PCM16, 48 kHz, 12,016,000 frames; 11,920,000 score frames plus the complete selected 96,000-frame (2-second) release tail.
- Last planned NoteOff: `occ395:mx_n11488`, P18, written event `mx_n11488`, 248.28333333333333 seconds, scheduled frame 11,917,600. Native response remains block-quantized.
- WAV SHA-256: `e74e472b2e21b9e09dccb536ec5b6f8b9d3ed34c2d2827d0ea3a3b167e523bf7`; 48,064,044 bytes. Independent WAV reading recovered every declared frame.
- Peak 0.0669173971; RMS 0.00612976705; final-50-ms peak 0.000000329578; no clipping/global silence or observed truncated-tail flag. Peak native voices 123; no NoteOff without a live voice. These are machine checks, not listening QA or proof that every patch's tail is complete.

The sealed attack-only receipt discloses 5,949 affected notes, 33 time-varying envelopes (32 paired steps and one accent-decay segment), and 5,916 constant nonunit envelopes. Thus the full compiler preserves numeric sustains and the editorial +16 accent decay, while this renderer deliberately omits envelope evolution and fractional gain multipliers. Slur/wedge/dashes/ornament omissions and the shared-GM pizzicato approximation remain explicit.

Local artifacts under `rehearsal-practice-probe/`: `III-app-selected.json`, `III-final-preflight.json`, `III-render.json`, `III-wav-audit.json`, and `III-practice.wav`. The final read-only boundary receipt's plan hash matches the pre-render preflight exactly. No second render was needed. III's authored attacca remains a later cross-movement clock/join decision, not permission to crop this standalone export.
