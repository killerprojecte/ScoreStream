# Official control checkpoint

Fresh isolated copy: scorestream-shadcn-official-20261114. Earlier previews and running instances remain untouched.

## Changes
- Installed only react-resizable-panels4.22.1 and @radix-ui/react-alert-dialog1.2.1 from registry.npmjs.org with install scripts disabled. DEPENDENCY_DELTA.json confirms exactly two added package entries, no existing-version changes and no new transitive packages.
- Imported official new-york-v4 Resizable and AlertDialog sources. Adaptations are imports only.20 registry-source hashes are checked by tests.
- Removed the manual PaneDivider component and its obsolete layout rules. Official Group/Panel/Separator own pointer, keyboard, focus and separator semantics. Source bounds remain220–680px; Inspector240–440px. Pixel preferences are retained, translated to percentages only for the Group API. Viewport/layout changes do not overwrite saved widths; only user-originated resize completion persists them. Programmatic reset restores the saved pixel defaults.
- Replaced temporary Dialog-based unsaved prompts with dedicated AlertDialog. Existing tested async close/open guards remain. Cancel gets focus; Escape cancels; outside interaction cannot accidentally accept or dismiss; repeated requests and unmounts preserve unsaved source.
- Only the two adjacent top-toolbar layout icons identified in image(2).png are changed to one standard shadcn TabsList, preserving Split/Score labels and actions. No text labels were added. The active content uses the matching Radix-generated panel relationship. Native window actions remain separate.
- Compact layout retains the existing official ToggleGroup and Source/Score/Inspector behavior. Its inactive content remains mounted so drafts are not discarded. The View menu, separate Inspector toggle and settings button are unchanged from the broad-stage baseline.
- Applied solid category tile fills based on inspected official Apple screenshots: gray#8B8B90, blue#3E8DF7, Sound pink-red#FC144B; green#309939 is a deliberate contrast-adjusted adaptation of sampled Apple Battery green. Existing white Lucide glyphs and24px tiles remain, without gradients or tile shadows. Detailed samples/coordinates/source URLs/contrast ratios: reference-apple-colors/SAMPLES.md.

## Inspector overview correction
The read-only value column no longer reserves46px/38px for editing actions. Label and value columns both use minmax(0,1fr), values align to the common right edge, and both can wrap long localized text. Existing group padding is20px on both sides. The new Resizable wrapper has no horizontal outer padding/gap in source; this is a code/composition check, not a native pixel measurement.

## Keyboard and rendering notes
Resizable uses the library's native accessible keyboard handling rather than the removed handmade10/40px key-step logic. No custom pointer or keyboard-resize implementation remains. Actual native interaction and pixel verification are still pending authorization; do not infer them from unit tests or build success.

## Validation
-134/134 frontend tests passed across17 files, including20 component provenance checks, AlertDialog cancellation/repeated-request/source-preservation tests, toolbar-tab ARIA relationships, compact ToggleGroup mounted-content preservation, read-only Inspector alignment composition, pixel conversion/bounds, and prior regressions.
- TypeScript and offline Vite production build passed. Existing bundle-size warning remains.
- Offline locked native Linux build passed; final status is recorded in native-build-official.log.
- No native launch or screenshot was attempted for this stage. Historical copied launcher files refer to prior previews and are not entry points for this checkpoint.
- No push, CI or unrelated dependency installation.

## Genuine non-shadcn boundaries
Monaco editing, Verovio notation/SVG note hit targets, real AppKit traffic lights, OS file dialogs, Linux native resize-edge targets, browser beforeunload, and decorative material/layout surfaces remain specialized integrations. They are not replacement app form controls.
