This file describes the previous broad-stage baseline. See SHADCN_OFFICIAL_CHECKPOINT.md for the current Resizable, AlertDialog, shared Tabs, palette and verification status.

# ScoreStream official control conversion

Isolated follow-on to the delivered toggle/icon correction. It does not overwrite any prior preview or running app.

## Implemented
- Toolbar and compact workspace views: official ToggleGroup / ToggleGroupItem / Toggle / Button; direct standard variants and sizes. No feature-specific state, radius, thumb translation or highlight rules.
- App menus: Menubar. Mutually exclusive view selection uses MenubarRadioGroup, while independent preferences use CheckboxItem.
- Source actions and diagnostics: Button; Monaco remains the source editor.
- Zoom controls and other icon helps: Button plus Tooltip. Renderer output remains Verovio.
- Inspector: official InputGroup/InputGroupInput/Addon/Text/Button; Button asChild in CollapsibleTrigger; Tooltip and Alert. Existing staging, validation, async commit and focus behavior retained.
- Preferences: Dialog, vertical Tabs, Select, Switch, Label and Button. Font size includes every valid integer10–22 and retains an existing fractional preference. Category icon tiles are noninteractive24px solid-color decorations with existing16px Lucide glyphs, no gradients or shadows.
- MCP settings: Input, Button, Alert and Spinner. Port uses official text Input with numeric inputMode and existing range validation, avoiding native tiny number spinner.
- Warnings, empty/loading UI: Alert, Empty and Spinner.
- App-owned unsaved prompts: existing official Dialog and Button, with tested asynchronous confirmation and a synchronous native close preventDefault guard. Browser beforeunload remains browser-owned.
- Linux/Windows caption buttons: official Button and existing Lucide icons, calling the same OS integration. macOS traffic lights remain actual AppKit controls.

## Not yet converted
- PaneDivider's custom splitter behavior remains pending permission for react-resizable-panels@4.22.1 required by official shadcn Resizable.
- Dedicated AlertDialog source can replace current official Dialog composition after permission for @radix-ui/react-alert-dialog@1.2.1. No npm dependency was added in this stage.

## Genuine exceptions
- Layout/semantic wrappers, score paper and noninteractive colored category tiles are application composition, not replacement interactive primitives.
- Monaco and Verovio are specialized editor/engraving integrations; Verovio note SVG hit targets remain accessible note-selection surfaces.
- Native AppKit traffic lights, Linux resize-edge hit targets and OS file dialogs remain platform integrations.
- Browser beforeunload is browser-owned and cannot be replaced by asynchronous React UI.
- Existing glass wrappers/material planes are decorative. Optical-library integration is being validated in another isolated copy.

## Registry integrity and compatibility
18 checked-in primitives have recorded official source and local SHA256 hashes in desktop/licenses/shadcn-registry-provenance.json. Adaptations change only cn/component/Radix import paths. No registry control geometry or behavior was rewritten. Root shadcn-provenance.json mirrors the authoritative manifest. components.json retains the schema-supported CLI style name new-york, while provenance identifies actual new-york-v4 sources.

The existing project uses Tailwind3.4.19. A small compatibility layer supplies existing v4 aliases (shadow-xs, rounded-xs, spacing variable, field-sizing-content, portal transform origins/available height and Alert description color). ToggleGroup receives the standard gap-1 composition class because the v4 spacing function is not interpreted by Tailwind3. No package upgrades performed.

## Validation so far
- Full frontend suite:125/125 tests in14 files passed, including18 registry hashes and new raw-control coverage tests.
- TypeScript and offline Vite production build passed. Existing bundle-size warning remains.
- Offline locked native Linux debug build passed.
- Native visual verification for this broad stage is queued. The preceding toggle/icon stage has already been visually inspected and measured natively.
- No remote writes, CI or prior-window closes.
