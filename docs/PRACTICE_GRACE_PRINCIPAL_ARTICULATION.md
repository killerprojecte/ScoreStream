# Explicit practice principal articulation composition

`GraceSoundPolicy::PracticePrincipalStaccatoAccentInAllocatedRemainderV1`
requires the existing explicit practice joint profile, selected
`SimpleStaccatoAccentV1`, and exact grace/hold policies. Existing strict and
ordinary grace policies are unchanged and still reject this combination.

Each grace retains its complete authored-order allocation and onset. Principal
staccato gates **one half of the remaining nominal interval**, not half of the
already 23/25-gated duration. Unmarked/accent-only principals retain 23/25.
Accent uses only the principal's actual fresh onset through existing expression
code. Original source and written markers remain unchanged. Selected hold
anchors, relations/ties affecting grace/principal, numeric-accent conflicts and
true hold crossings retain their existing rejection. Articulations on grace
notes themselves are still unsupported: this slice does not invent a gate for
an actual source family not present in the verified IV evidence.

Verified movement IV source SHA256:
`39044fe2ed9a3d138b67b3c5d5db64794dfb64627d07258bc0854658017f71c9`.
The exact source/route census finds 112 grace occurrences, 48 principals with a
single staccato (32 distinct written principals), no marked grace notes and no
accent in these affected groups. First: P1 m11, `occ10:mx_n55` grace followed by
`occ10:mx_n56` principal, nominal quarter interval 40..81/2. Last: P19 m220,
`occ302:mx_n22295` followed by `occ302:mx_n22296`, 1154..2309/2.

`probe_practice SOURCE POLICY continue-to-movement-end practice-principal-articulation`
selects this policy explicitly and reports the exact resulting sound windows.
This is a pure core timing probe, not a source-bound native rendering receipt.
No performance fidelity or completed IV audio is claimed.
