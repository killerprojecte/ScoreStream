#!/usr/bin/env python3
"""Build and validate the planned MCP contract bundle; not product/server code.

Reads the authoritative format, selection, and edit contracts without modifying
them. All tool schemas are emitted with local $defs only. No network access or
application operation is performed. Requires the packaging dependency jsonschema.
"""
from __future__ import annotations

import copy
import json
from pathlib import Path

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource

ROOT = Path(__file__).resolve().parents[1]
DIALECT = "https://json-schema.org/draft/2020-12/schema"
COMMON = "urn:scorestream:schema:common:1.1"
SELECTION = "urn:scorestream:schema:selection:1.1"
EDIT = "urn:scorestream:schema:edit:1.1"
FORMAT = "urn:scorestream:schema:format-reference:1.1"
JOB = "urn:scorestream:schema:job:1.1"
RENDER = "urn:scorestream:schema:render:1.1"
CONTRACT = "urn:scorestream:schema:mcp-contract:1.1"


def read(path):
    return json.loads((ROOT / path).read_text())


def write(path, value):
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def obj(properties, required=None, **extra):
    return {
        "type": "object", "properties": properties,
        "required": list(properties) if required is None else required,
        "additionalProperties": False, **extra,
    }


def string(**extra):
    return {"type": "string", **extra}


def array(items, maximum=256, **extra):
    return {"type": "array", "items": items, "maxItems": maximum, **extra}


def ref(document, name=None):
    return {"$ref": document + ("#/$defs/" + name if name else "")}


def const(value):
    return {"const": value}


def enum(*values, **extra):
    return {"type": "string", "enum": list(values), **extra}


ID = ref(COMMON, "Id")
REVISION = ref(COMMON, "RevisionId")
SCORE = ref(COMMON, "ScoreRef")
DIAGNOSTICS = array(ref(COMMON, "Diagnostic"), 100)
URI = string(format="uri", pattern=r"^scorestream://", maxLength=2048)
HASH = string(pattern=r"^[0-9a-f]{64}$", minLength=64, maxLength=64)
PROFILE = const("core-measured")
DATE = string(format="date-time")


def document(identifier, title, definitions, root):
    return {
        "$schema": DIALECT, "$id": identifier, "title": title,
        "$ref": "#/$defs/" + root, "$defs": definitions,
    }


def build_support_schemas():
    job_defs = {}
    job_defs["DomainError"] = obj({
        "code": string(pattern=r"^[a-z][a-z0-9_]*$", maxLength=100),
        "message": string(minLength=1, maxLength=4096),
        "retryable": {"type": "boolean"},
        "diagnostics": DIAGNOSTICS,
        "score_id": ID,
        "expected_revision_id": REVISION,
        "current_revision_id": REVISION,
        "details_uri": URI,
    }, ["code", "message", "retryable", "diagnostics"],
        description="An actionable application error, not a JSON-RPC error code. Diagnostics are bounded; additional details may be a resource.")
    job_defs["ArtifactRef"] = obj({
        "artifact_id": ID, "uri": URI, "mime_type": string(maxLength=160),
        "size_bytes": {"type": "integer", "minimum": 0}, "sha256": HASH,
        "expires_at": DATE,
    }, ["artifact_id", "uri", "mime_type", "size_bytes", "sha256"])
    states = enum("queued", "running", "cancel_requested", "succeeded", "failed", "cancelled")
    kinds = enum("score_import", "score_export", "score_render")
    job_defs["JobSubmission"] = obj({
        "kind": const("application_job"), "job_id": ID, "operation": kinds,
        "status_uri": URI,
        "poll_after_ms": {"type": "integer", "minimum": 250, "maximum": 60000},
        "expires_at": DATE,
    }, description="A durable application job handle. job_id is not an MCP taskId; it is used only with application job tools/resources.")
    job_defs["ApplicationJob"] = obj({
        "job_id": ID, "operation": kinds, "state": states,
        "created_at": DATE, "updated_at": DATE, "expires_at": DATE,
        "poll_after_ms": {"type": "integer", "minimum": 250, "maximum": 60000},
        "source_score": SCORE,
        "stage": string(maxLength=200),
        "progress": obj({
            "completed": {"type": "integer", "minimum": 0},
            "total": {"type": "integer", "minimum": 0},
        }, ["completed"]),
        "result_uri": URI, "result_schema_uri": URI,
        "error": ref(JOB, "DomainError"),
    }, ["job_id", "operation", "state", "created_at", "updated_at", "expires_at", "poll_after_ms"],
        allOf=[
            {"if": {"properties": {"state": const("succeeded")}}, "then": {"required": ["result_uri", "result_schema_uri"]}, "else": {"not": {"anyOf": [{"required": ["result_uri"]}, {"required": ["result_schema_uri"]}]}}},
            {"if": {"properties": {"state": const("failed")}}, "then": {"required": ["error"]}, "else": {"not": {"required": ["error"]}}},
        ], description="Result resources contain the original tool's final application envelope and are validated by result_schema_uri. These application states are not MCP extension task states.")
    job_defs["JobQuery"] = obj({"job_id": ID})
    job_defs["JobCancelRequest"] = obj({"job_id": ID, "reason": string(maxLength=256)}, ["job_id"])
    job_defs["JobCancellation"] = obj({
        "job_id": ID, "state": states, "cancellation_requested": {"type": "boolean"},
        "status_uri": URI,
    }, description="The observed result of a cooperative cancellation request. Terminal jobs may return cancellation_requested=false; no committed revision is undone.")
    write("schemas/job.schema.json", document(JOB, "ScoreStream application job contract 1.1", job_defs, "ApplicationJob"))

    render_defs = {}
    render_defs["Coverage"] = obj({
        "interpreted_features": array(string(maxLength=160)),
        "preserved_only_features": array(string(maxLength=160)),
        "unsupported_features": array(string(maxLength=160)),
        "complete": {"type": "boolean"},
        "details_uri": URI,
    }, ["interpreted_features", "preserved_only_features", "unsupported_features", "complete"],
        description="Capability observations for this operation and revision, not a claim of universal MusicXML completeness. complete describes this bounded report, not total semantic coverage.")
    render_defs["LayoutOptions"] = obj({
        "mode": enum("page", default="page"),
        "page_width_mm": {"type": "number", "minimum": 50, "maximum": 1000, "default": 210},
        "page_height_mm": {"type": "number", "minimum": 50, "maximum": 1000, "default": 297},
    }, [], description="Physical page dimensions. They are translated by the backend; returned SVG viewBox coordinates are a separate space. A UI zoom is not a score edit.")
    render_defs["RenderRequest"] = obj({
        "score_id": ID, "revision_id": REVISION,
        "selection": ref(SELECTION, "FrozenSelectionRef"),
        "layout": ref(RENDER, "LayoutOptions"),
    }, ["score_id", "revision_id"], description="Requires a configured available backend. Optional selection must name this same score revision. Whole score is the default scope.")
    render_defs["Page"] = obj({
        "page_id": ID, "page_number": {"type": "integer", "minimum": 1},
        "svg_uri": URI, "mapping_uri": URI,
        "coordinate_space": const("page_viewbox"),
        "view_box": ref(SELECTION, "PageRectangle"),
    }, description="Page numbering is presentation; page_id is scoped to render_id. The SVG and mapping are published together.")
    render_defs["RenderResult"] = obj({
        "kind": const("render"), "render_id": ID, "score_id": ID, "revision_id": REVISION,
        "options_hash": HASH,
        "renderer": obj({"name": string(maxLength=100), "version": string(maxLength=100), "projection_version": string(maxLength=100), "font_set_id": string(maxLength=160)}),
        "manifest_uri": URI, "total_pages": {"type": "integer", "minimum": 1},
        "pages": array(ref(RENDER, "Page"), 256, minItems=1),
        "pages_complete": {"type": "boolean"}, "pages_continuation_uri": URI,
        "coverage": ref(RENDER, "Coverage"), "diagnostics": DIAGNOSTICS,
    }, ["kind", "render_id", "score_id", "revision_id", "options_hash", "renderer", "manifest_uri", "total_pages", "pages", "pages_complete", "coverage", "diagnostics"],
        allOf=[{"if": {"properties": {"pages_complete": const(False)}}, "then": {"required": ["pages_continuation_uri"]}, "else": {"not": {"required": ["pages_continuation_uri"]}}}],
        description="A derived immutable result for an exact score revision. Further immutable page-index resources bound a large result; stale UI views must not publish this result as another revision.")
    render_defs["MappedObject"] = obj({
        "svg_id": string(minLength=1, maxLength=200),
        "object_ids": array(ID, 64, uniqueItems=True),
        "object_kind": enum("event", "measure", "relation", "mark", "layout_only"),
        "bounds": array(ref(SELECTION, "PageRectangle"), 64),
        "editable": {"type": "boolean"},
    }, description="Mapping is not assumed one-to-one. Layout-only objects may have no musical object IDs. Visibility does not grant edit authority.")
    render_defs["PageMapping"] = obj({
        "render_id": ID, "score_id": ID, "revision_id": REVISION, "page_id": ID,
        "coordinate_space": const("page_viewbox"),
        "objects": array(ref(RENDER, "MappedObject"), 10000),
        "complete": {"type": "boolean"}, "continuation_uri": URI,
    }, ["render_id", "score_id", "revision_id", "page_id", "coordinate_space", "objects", "complete"],
        allOf=[{"if": {"properties": {"complete": const(False)}}, "then": {"required": ["continuation_uri"]}}])
    write("schemas/render.schema.json", document(RENDER, "ScoreStream revision-specific rendering contract 1.1", render_defs, "RenderResult"))


def build_domain_schemas():
    full = read("schemas/format-reference.schema.json")
    quick = copy.deepcopy(full)
    quick.pop("$id", None)
    quick.pop("$schema", None)
    quick["title"] = "Quick authoring reference returned by default"
    quick["required"] = [x for x in quick["required"] if x != "vocabulary"] + ["common_attributes", "core_shape", "pitch", "time"]
    quick["properties"].update({
        "common_attributes": {"type": "object"}, "core_shape": string(minLength=1),
        "pitch": {"type": "object"}, "time": {"type": "object"},
    })
    definitions = {"QuickReference": quick}
    definitions["FormatRequest"] = obj({
        "format_version": {"type": "string", "const": "1.1", "default": "1.1"},
        "profile": {"type": "string", "const": "core-measured", "default": "core-measured"},
        "detail": enum("quick", "full", default="quick"),
        "topics": array(enum("syntax", "pitch", "time", "polyphony", "state", "marks", "relations", "editing", "extensions", "validation"), 10, uniqueItems=True),
    }, [], description="Empty arguments return current quick syntax and examples inline. Topics add requested detail without omitting the minimum authoring contract. Selection of unsupported versions/profiles is an actionable tool error.")
    definitions["FormatData"] = {"anyOf": [ref(CONTRACT, "QuickReference"), ref(FORMAT)]}
    inline = obj({"kind": const("inline"), "format": enum("sfs", "musicxml"), "text": string(minLength=1, maxLength=1048576)})
    asset = obj({"kind": const("asset"), "format": enum("sfs", "musicxml", "mxl"), "asset_id": ID})
    definitions["ImportRequest"] = obj({
        "source": {"oneOf": [inline, asset]}, "idempotency_key": ID,
        "profile": {**PROFILE, "default": "core-measured"},
        "fidelity_policy": enum("strict", "preserve_uninterpreted", default="preserve_uninterpreted"),
    }, ["source", "idempotency_key"], description="Text or registered assets only. No network fetch or caller-supplied filesystem path. Preserved source payload is explicitly distinguished from interpreted score semantics.")
    definitions["ImportResult"] = obj({
        "kind": const("imported_score"), "score": SCORE, "format_version": const("1.1"),
        "profile": PROFILE, "manifest_uri": URI, "source_asset_id": ID,
        "coverage": ref(RENDER, "Coverage"), "diagnostics": DIAGNOSTICS,
    }, ["kind", "score", "format_version", "profile", "manifest_uri", "coverage", "diagnostics"])
    definitions["InspectRequest"] = obj({"score_id": ID, "revision_id": REVISION}, ["score_id"])
    definitions["PartSummary"] = obj({"part_id": ID, "alias": string(minLength=1, maxLength=160), "instrument": string(maxLength=160), "staff_ids": array(ID, 64, minItems=1, uniqueItems=True)})
    definitions["VoiceSummary"] = obj({"voice_id": ID, "alias": string(minLength=1, maxLength=160), "part_id": ID, "default_staff_id": ID})
    definitions["InspectResult"] = obj({
        "score": SCORE, "head_revision_id": REVISION, "format_version": const("1.1"), "profile": PROFILE,
        "parts": array(ref(CONTRACT, "PartSummary")), "voices": array(ref(CONTRACT, "VoiceSummary"), 1024),
        "measure_count": {"type": "integer", "minimum": 0}, "manifest_uri": URI,
        "coverage": ref(RENDER, "Coverage"), "diagnostics": DIAGNOSTICS,
    })
    definitions["RegionContext"] = obj({
        "declarations_sfs": string(maxLength=262144),
        "entry_state_sfs": string(maxLength=262144),
        "continuing_event_ids": array(ID, 4096, uniqueItems=True),
        "relation_ids": array(ID, 4096, uniqueItems=True),
        "complete": {"type": "boolean"}, "resource_uri": URI,
    }, ["declarations_sfs", "entry_state_sfs", "continuing_event_ids", "relation_ids", "complete"],
        allOf=[{"if": {"properties": {"complete": const(False)}}, "then": {"required": ["resource_uri"]}}],
        description="Read-only context, not edit authorization. Full referenced context must be fetched before an edit depending on it; missing context is never guessed.")
    definitions["TextObjectBinding"] = obj({
        "text_field": enum("fragment_sfs", "declarations_sfs", "entry_state_sfs"),
        "object_kind": enum("event", "measure", "relation", "mark", "chord", "part", "voice", "staff"),
        "object_id": ID,
        "text_id": ref(COMMON, "TextIdentifier"),
        "line_1based": {"type": "integer", "minimum": 1},
    }, ["text_field", "object_kind", "object_id"],
        anyOf=[{"required": ["text_id"]}, {"required": ["line_1based"]}],
        description="An explicit alias and/or source-line binding within the named snippet. A line is required when an object has no SFS alias, such as an unlabelled intrinsic mark. Optional lines count from one in that exact text field. Never infer an opaque object ID from event order or the spelling of a local alias.")
    definitions["TextObjectMap"] = obj({
        "score": SCORE,
        "entries": array(ref(CONTRACT, "TextObjectBinding"), 20000),
        "complete": {"type": "boolean"},
        "resource_uri": URI,
    }, ["score", "entries", "complete"],
        allOf=[{"if": {"properties": {"complete": const(False)}}, "then": {"required": ["resource_uri"]}}],
        description="Revision-bound mappings for fragment and context snippets, including every supplied referencable alias and relevant unlabelled object. The score must equal FrozenSelection.score. If incomplete, fetch the full map resource before identity-dependent edits; neither missing entries nor context bindings may be guessed.")
    definitions["RegionFragment"] = obj({
        "span": ref(SELECTION, "ResolvedSpan"),
        "fragment_sfs": string(minLength=1, maxLength=1048576),
        "voice_bindings": array(ref(EDIT, "VoiceBinding"), 1024, minItems=1),
        "replacement_complete": {"type": "boolean"},
        "context": ref(CONTRACT, "RegionContext"),
        "text_map": ref(CONTRACT, "TextObjectMap"),
    }, description="An inspection fragment normalized to local time. An event-only or boundary-crossing selection is not necessarily a complete replace_range payload; replacement_complete explicitly states that distinction.")
    definitions["ReadRegionResult"] = obj({
        "selection": ref(SELECTION, "FrozenSelection"),
        "fragments": array(ref(CONTRACT, "RegionFragment"), 256),
        "diagnostics": DIAGNOSTICS,
    }, description="The selection is frozen and never silently truncated or re-evaluated against a newer revision. Oversized selection/results return a resource_limit domain error, not incomplete edit authority.")
    definitions["ValidateRequest"] = obj({"score_id": ID, "revision_id": REVISION, "profile": {**PROFILE, "default": "core-measured"}}, ["score_id", "revision_id"])
    definitions["ValidateResult"] = obj({"score": SCORE, "valid": const(True), "profile": PROFILE, "coverage": ref(RENDER, "Coverage"), "diagnostics": DIAGNOSTICS})
    preview = copy.deepcopy(read("schemas/edit.schema.json")["$defs"]["EditRequest"])
    preview["properties"].pop("idempotency_key")
    preview["required"].remove("idempotency_key")
    preview["description"] = "Validate an ephemeral candidate with the same selection/base/operations as EditRequest. No commit and no approval token is required. The referenced frozen selection and expected revision must agree."
    preview["properties"]["operations"]["items"] = ref(EDIT, "Operation")
    definitions["PreviewRequest"] = preview
    definitions["PreviewResult"] = obj({
        "status": const("preview"), "score_id": ID, "expected_revision_id": REVISION,
        "would_commit": const(True), "changed_ids": array(ID, 4096, uniqueItems=True),
        "deleted_ids": array(ID, 4096, uniqueItems=True),
        "planned_insertions": {"type": "integer", "minimum": 0},
        "affected_relation_ids": array(ID, 4096, uniqueItems=True), "diagnostics": DIAGNOSTICS,
    }, description="A pure validation/diff preview; no artifact or durable job is created by this MVP tool. It neither reserves new IDs nor changes head. Rendering a draft is a desktop application function.")
    definitions["ExportRequest"] = obj({
        "score_id": ID, "revision_id": REVISION, "format": enum("sfs", "sfp", "musicxml", "mxl"),
        "fidelity_policy": enum("strict", "allow_reported_loss", default="strict"),
    }, ["score_id", "revision_id", "format"], description="Export a pinned revision. Adapter availability comes from the deployed capability matrix; unsupported formats fail. Loss is allowed only when the request explicitly opts in.")
    definitions["ExportResult"] = obj({
        "kind": const("exported_score"), "score": SCORE, "format": enum("sfs", "sfp", "musicxml", "mxl"),
        "artifact": ref(JOB, "ArtifactRef"), "lossy": {"type": "boolean"},
        "coverage": ref(RENDER, "Coverage"), "diagnostics": DIAGNOSTICS,
    })
    write("schemas/mcp-contract.schema.json", document(CONTRACT, "Auxiliary planned MCP data contracts 1.1", definitions, "FormatRequest"))


class Bundler:
    """Resolve only the checked-in schema registry; never dereference a URL."""

    def __init__(self, docs):
        self.docs = docs
        self.defs = {}
        self.names = {}

    def resolve(self, reference, base=None):
        origin, separator, pointer = reference.partition("#")
        origin = origin or base
        if origin not in self.docs:
            raise ValueError(f"Unregistered schema reference: {reference} (base {base})")
        value = self.docs[origin]
        if separator and pointer:
            if not pointer.startswith("/"):
                raise ValueError("Only JSON Pointer schema fragments are supported")
            for token in pointer[1:].split("/"):
                value = value[token.replace("~1", "/").replace("~0", "~")]
        return value, origin, pointer

    def reference(self, reference, base):
        value, origin, pointer = self.resolve(reference, base)
        key = (origin, pointer)
        if key not in self.names:
            short = origin.split(":")[-2].replace("-", "_")
            leaf = pointer.rsplit("/", 1)[-1] if pointer else "Root"
            name = short + "__" + leaf
            if name in self.defs:
                raise ValueError(f"Bundled definition collision: {name}")
            self.names[key] = name
            self.defs[name] = {}  # Reserve before recursion.
            self.defs[name] = self.transform(value, origin)
        return "#/$defs/" + self.names[key]

    def transform(self, value, base):
        if isinstance(value, list):
            return [self.transform(item, base) for item in value]
        if not isinstance(value, dict):
            return value
        base = value.get("$id", base)
        result = {}
        for key, item in value.items():
            if key in ("$id", "$schema", "$defs"):
                continue
            result[key] = self.reference(item, base) if key == "$ref" else self.transform(item, base)
        return result

    def bundle(self, schema, identifier, title, base=None, output=False):
        if set(schema) == {"$ref"}:
            schema, base, _ = self.resolve(schema["$ref"], base)
        body = self.transform(schema, base)
        result = {"$schema": DIALECT, "$id": identifier, "title": title, **body}
        if self.defs:
            result["$defs"] = dict(sorted(self.defs.items()))
        if output:
            bound_diagnostics(result)
        return result


def bound_diagnostics(value):
    if isinstance(value, list):
        for item in value:
            bound_diagnostics(item)
    elif isinstance(value, dict):
        properties = value.get("properties", {})
        if "diagnostics" in properties and properties["diagnostics"].get("type") == "array":
            properties["diagnostics"]["maxItems"] = 100
        for item in value.values():
            bound_diagnostics(item)


def envelope(data):
    return {
        "type": "object",
        "oneOf": [obj({"ok": const(True), "data": data}), obj({"ok": const(False), "error": ref(JOB, "DomainError")})],
        "description": "Exactly one application success or error branch. Protocol resultType/content/structuredContent/isError wrap this object; they are not fields inside it.",
    }


def build_catalog(docs):
    plans = [
        ("score_get_format", "Get the ScoreStream language standard", ref(CONTRACT, "FormatRequest"), ref(CONTRACT, "FormatData"), True, False, True,
         "Call first to learn exact SFS syntax, scopes, rational time, vocabulary, examples, and versioned format resources. Empty arguments return the current quick reference inline; this is the score-format standard, not merely tool JSON Schema."),
        ("score_import", "Import a score", ref(CONTRACT, "ImportRequest"), {"oneOf": [ref(CONTRACT, "ImportResult"), ref(JOB, "JobSubmission")]}, False, False, True,
         "Create a score from inline SFS/MusicXML or a registered asset. MXL requires an asset. The principal, idempotency key, and normalized request deduplicate retries during the advertised retention period. No remote fetch occurs; uninterpreted source payload remains explicitly identified."),
        ("score_inspect", "Inspect score identity and revision", ref(CONTRACT, "InspectRequest"), ref(CONTRACT, "InspectResult"), True, False, True,
         "Inspect an exact revision, or discover head when revision_id is omitted. Returns the exact score/revision, part and voice IDs, coverage, and the immutable manifest URI. Use its returned revision for later reads and edits."),
        ("score_read_region", "Read and freeze a score region", ref(SELECTION, "SelectionRequest"), ref(CONTRACT, "ReadRegionResult"), False, False, True,
         "Resolve a SelectionRequest against its pinned score revision, register a content-addressed frozen selection, and return inspection fragments with entry/boundary context. Identical requests reuse the same immutable selection handle. No score revision changes; the selection registration is nevertheless an application effect."),
        ("score_validate", "Validate an immutable score revision", ref(CONTRACT, "ValidateRequest"), ref(CONTRACT, "ValidateResult"), True, False, True,
         "Validate core semantic invariants for the pinned revision/profile. Invalid scores use the error branch with diagnostics. This is not a claim of complete MusicXML support, and JSON Schema validation alone is insufficient."),
        ("score_preview_edit", "Preview typed score edits", ref(CONTRACT, "PreviewRequest"), ref(CONTRACT, "PreviewResult"), True, False, True,
         "Run the same typed edit validator against an ephemeral candidate and return the prospective change summary. No score commit, artifact, job, or approval gate is created. The frozen selection and expected revision must agree."),
        ("score_apply_edit", "Commit an atomic score edit", ref(EDIT, "EditRequest"), ref(EDIT, "EditResult"), False, True, True,
         "Apply the exact shared EditRequest directly. Atomically compare expected_revision_id, validate the frozen selection and operations, and commit one new immutable revision or none. Same-principal idempotency retries return the original result. Stale heads and unresolved crossings fail; no silent rebase or ripple."),
        ("score_export", "Export an immutable score revision", ref(CONTRACT, "ExportRequest"), {"oneOf": [ref(CONTRACT, "ExportResult"), ref(JOB, "JobSubmission")]}, False, False, False,
         "Export a pinned score revision using a configured supported adapter. Strict fidelity is the default. Writes derived artifacts and possibly an application job; the source score is unchanged. Returns explicit loss/coverage diagnostics and resource references."),
        ("score_render", "Render notation for a pinned revision", ref(RENDER, "RenderRequest"), {"oneOf": [ref(RENDER, "RenderResult"), ref(JOB, "JobSubmission")]}, False, False, False,
         "Available only when a desktop-attached or headless renderer backend is configured and available. Creates SVG/mapping artifacts or an application job for the exact revision/options. It does not edit score data or control playback; stale results must not replace a newer UI view."),
        ("job_get", "Get application job status", ref(JOB, "JobQuery"), ref(JOB, "ApplicationJob"), True, False, True,
         "Read a durable application job using job_id. Terminal successful jobs identify the original tool result resource and schema. This is not tasks/get, and an MCP taskId is not accepted as an application job ID."),
        ("job_cancel", "Request application job cancellation", ref(JOB, "JobCancelRequest"), ref(JOB, "JobCancellation"), False, True, True,
         "Request cooperative cancellation of the named application job. Repeated requests are harmless. Return the observed state; cancellation may race completion and never undoes a committed score revision. This is not tasks/cancel."),
    ]
    tools = []
    for name, title, inputs, data, read_only, destructive, idempotent, description in plans:
        entry = {"name": name, "title": title, "description": description}
        entry["inputSchema"] = Bundler(docs).bundle(inputs, f"urn:scorestream:tool:{name}:input:1.1", title + " input")
        entry["outputSchema"] = Bundler(docs).bundle(envelope(data), f"urn:scorestream:tool:{name}:output:1.1", title + " application result", output=True)
        entry["annotations"] = {"readOnlyHint": read_only, "destructiveHint": destructive, "idempotentHint": idempotent, "openWorldHint": False}
        tools.append(entry)
        write(f"schemas/mcp/{name}.input.schema.json", entry["inputSchema"])
        write(f"schemas/mcp/{name}.output.schema.json", entry["outputSchema"])
    # Stable order is intentional; the discovery operation remains first.
    write("mcp/tools-list.planned.json", {"resultType": "complete", "tools": tools})
    write("mcp/deployment-notes.json", {
        "status": "design_only_no_running_server", "protocol_baseline": "2026-07-28", "legacy_adapter": "2025-11-25",
        "catalog_file": "mcp/tools-list.planned.json", "central_discovery_tool": "score_get_format",
        "conditional_tools": {"score_render": "Advertise only with an implemented available desktop-attached or headless renderer; a protocol handler alone is insufficient."},
        "publication_rule": "Filter this planned catalog to implemented, configured and authorized handlers. No stub operations are available simply because they appear here.",
        "withheld_families": ["annotations", "knowledge_search", "model_generation", "performance_rendering", "audio_synthesis", "playback_control"],
        "application_jobs": "job_id belongs to job_get/job_cancel and the job resources. It is not an MCP taskId.",
        "schema_rule": "Every embedded tool schema is self-contained JSON Schema 2020-12; no external $ref dereference is needed.",
    })
    return tools


def build_examples(tools):
    coverage = {"interpreted_features": ["pitched_notes", "voices", "rational_time"], "preserved_only_features": [], "unsupported_features": [], "complete": True}
    score = {"score_id": "score_demo", "revision_id": "rev_001"}
    manifest_uri = "scorestream://scores/score_demo/revisions/rev_001/manifest"
    selection_request = read("examples/auxiliary/valid/selection-time-range.json")
    selection = read("examples/auxiliary/valid/selection-frozen.json")
    edit = read("examples/auxiliary/valid/edit-set-pitch.json")
    preview = {k: copy.deepcopy(v) for k, v in edit.items() if k != "idempotency_key"}
    imported_source = "!score 1.1 unit=q\n!part p instrument=piano staves=2\n!voice pr part=p staff=1\n!voice pl part=p staff=2\n@0\n= score tempo=q:120\n= p.s1 meter=4/4 key=0 clef=G2\n= p.s2 meter=4/4 key=0 clef=F4\n| p measure_p1 len=4 label=\"1\"\npr C4:4 id=event_c4\npl E4:4 id=event_e4\n.\n!end at=4\n"
    fixture_path = ROOT / "examples/mcp/score-demo.sfs"
    fixture_path.parent.mkdir(parents=True, exist_ok=True)
    fixture_path.write_text(imported_source)
    text_map = {"score": score, "complete": True, "entries": [
        {"text_field": "fragment_sfs", "object_kind": "event", "text_id": "event_c4", "object_id": "event_c4", "line_1based": 3},
        {"text_field": "fragment_sfs", "object_kind": "event", "text_id": "event_e4", "object_id": "event_e4", "line_1based": 4},
        {"text_field": "fragment_sfs", "object_kind": "voice", "text_id": "right", "object_id": "voice_pr"},
        {"text_field": "fragment_sfs", "object_kind": "voice", "text_id": "left", "object_id": "voice_pl"},
        {"text_field": "declarations_sfs", "object_kind": "part", "text_id": "p", "object_id": "part_p", "line_1based": 1},
        {"text_field": "declarations_sfs", "object_kind": "voice", "text_id": "right", "object_id": "voice_pr", "line_1based": 2},
        {"text_field": "declarations_sfs", "object_kind": "voice", "text_id": "left", "object_id": "voice_pl", "line_1based": 3},
        {"text_field": "entry_state_sfs", "object_kind": "part", "text_id": "p", "object_id": "part_p"},
        {"text_field": "entry_state_sfs", "object_kind": "staff", "object_id": "staff_p1", "line_1based": 2},
        {"text_field": "entry_state_sfs", "object_kind": "staff", "object_id": "staff_p2", "line_1based": 3},
    ]}
    render_data = {
        "kind": "render", "render_id": "render_demo", **score, "options_hash": "b" * 64,
        "renderer": {"name": "configured-test-backend", "version": "design-fixture", "projection_version": "1.1", "font_set_id": "fixture-fonts"},
        "manifest_uri": "scorestream://renders/render_demo/manifest", "total_pages": 1,
        "pages": [{"page_id": "page_1", "page_number": 1, "svg_uri": "scorestream://renders/render_demo/pages/page_1", "mapping_uri": "scorestream://renders/render_demo/pages/page_1/mapping", "coordinate_space": "page_viewbox", "view_box": {"x": 0, "y": 0, "width": 1000, "height": 1400}}],
        "pages_complete": True, "coverage": coverage, "diagnostics": [],
    }
    job_data = {"job_id": "job_render_001", "operation": "score_render", "state": "succeeded", "created_at": "2026-11-13T09:00:00Z", "updated_at": "2026-11-13T09:00:01Z", "expires_at": "2026-11-14T09:00:00Z", "poll_after_ms": 1000, "source_score": score, "stage": "complete", "result_uri": "scorestream://jobs/job_render_001/result", "result_schema_uri": "scorestream://schemas/1.1/score_render.output"}
    examples = {
        "score_get_format": ({}, None),
        "score_import": ({"source": {"kind": "inline", "format": "sfs", "text": imported_source}, "idempotency_key": "import_demo_001"}, {"kind": "imported_score", "score": score, "format_version": "1.1", "profile": "core-measured", "manifest_uri": manifest_uri, "coverage": coverage, "diagnostics": []}),
        "score_inspect": ({"score_id": "score_demo"}, {"score": score, "head_revision_id": "rev_001", "format_version": "1.1", "profile": "core-measured", "parts": [{"part_id": "part_p", "alias": "p", "instrument": "piano", "staff_ids": ["staff_p1", "staff_p2"]}], "voices": [{"voice_id": "voice_pr", "alias": "pr", "part_id": "part_p", "default_staff_id": "staff_p1"}, {"voice_id": "voice_pl", "alias": "pl", "part_id": "part_p", "default_staff_id": "staff_p2"}], "measure_count": 1, "manifest_uri": manifest_uri, "coverage": coverage, "diagnostics": []}),
        "score_read_region": (selection_request, {"selection": selection, "fragments": [{"span": selection["resolved_spans"][0], "fragment_sfs": "!fragment 1.1 unit=q span=4\n@0\nright C4:4 id=event_c4\nleft E4:4 id=event_e4\n.\n!end at=4\n", "voice_bindings": [{"fragment_voice": "right", "voice_id": "voice_pr"}, {"fragment_voice": "left", "voice_id": "voice_pl"}], "replacement_complete": True, "context": {"declarations_sfs": "!part p instrument=piano staves=2\n!voice right part=p staff=1\n!voice left part=p staff=2\n", "entry_state_sfs": "= score tempo=q:120\n= p.s1 meter=4/4 key=0 clef=G2\n= p.s2 meter=4/4 key=0 clef=F4\n", "continuing_event_ids": [], "relation_ids": [], "complete": True}, "text_map": text_map}], "diagnostics": []}),
        "score_validate": (score, {"score": score, "valid": True, "profile": "core-measured", "coverage": coverage, "diagnostics": []}),
        "score_preview_edit": (preview, {"status": "preview", "score_id": "score_demo", "expected_revision_id": "rev_001", "would_commit": True, "changed_ids": ["event_c4"], "deleted_ids": [], "planned_insertions": 0, "affected_relation_ids": [], "diagnostics": []}),
        "score_apply_edit": (edit, {"status": "committed", "score": {"score_id": "score_demo", "revision_id": "rev_002"}, "parent_revision_id": "rev_001", "command_id": "command_demo_001", "actor": {"kind": "agent", "principal_id": "principal_demo"}, "changed_ids": ["event_c4"], "created_ids": [], "deleted_ids": [], "lineage": [{"kind": "preserved", "before_ids": ["event_c4"], "after_ids": ["event_c4"]}], "invalidated_render_ids": ["render_demo"], "diagnostics": []}),
        "score_export": ({**score, "format": "sfs"}, {"kind": "exported_score", "score": score, "format": "sfs", "artifact": {"artifact_id": "artifact_demo", "uri": "scorestream://artifacts/artifact_demo", "mime_type": "text/plain", "size_bytes": len(imported_source.encode()), "sha256": __import__("hashlib").sha256(imported_source.encode()).hexdigest()}, "lossy": False, "coverage": coverage, "diagnostics": []}),
        "score_render": (score, render_data),
        "job_get": ({"job_id": "job_render_001"}, job_data),
        "job_cancel": ({"job_id": "job_render_002", "reason": "Superseded preview"}, {"job_id": "job_render_002", "state": "cancel_requested", "cancellation_requested": True, "status_uri": "scorestream://jobs/job_render_002"}),
    }
    cases = []
    for name, (arguments, data) in examples.items():
        arg_path = f"examples/mcp/valid/{name}.arguments.json"
        write(arg_path, arguments)
        cases.append({"file": arg_path, "tool": name, "schema": "inputSchema", "expected_valid": True})
        if name == "score_get_format":
            result_path = "mcp/examples/score_get_format.structured.json"
        else:
            result_path = f"examples/mcp/valid/{name}.structured.json"
            write(result_path, {"ok": True, "data": data})
        cases.append({"file": result_path, "tool": name, "schema": "outputSchema", "expected_valid": True})
    full_path = "examples/mcp/valid/score_get_format.full.structured.json"
    write(full_path, {"ok": True, "data": read("spec/scorestream-language.json")})
    cases.append({"file": full_path, "tool": "score_get_format", "schema": "outputSchema", "expected_valid": True})
    error = {"ok": False, "error": {"code": "revision_conflict", "message": "The head changed. Read rev_002 and deliberately re-plan this edit.", "retryable": False, "score_id": "score_demo", "expected_revision_id": "rev_001", "current_revision_id": "rev_002", "diagnostics": []}}
    error_path = "examples/mcp/valid/score_apply_edit.conflict.structured.json"
    write(error_path, error)
    cases.append({"file": error_path, "tool": "score_apply_edit", "schema": "outputSchema", "expected_valid": True})
    queued = {"ok": True, "data": {"kind": "application_job", "job_id": "job_render_003", "operation": "score_render", "status_uri": "scorestream://jobs/job_render_003", "poll_after_ms": 1000, "expires_at": "2026-11-14T09:00:00Z"}}
    job_path = "examples/mcp/valid/score_render.job.structured.json"
    write(job_path, queued)
    cases.append({"file": job_path, "tool": "score_render", "schema": "outputSchema", "expected_valid": True})
    mxl_path = "examples/mcp/valid/score_import.asset-mxl.arguments.json"
    write(mxl_path, {"source": {"kind": "asset", "format": "mxl", "asset_id": "asset_source_mxl"}, "idempotency_key": "import_mxl_demo"})
    cases.append({"file": mxl_path, "tool": "score_import", "schema": "inputSchema", "expected_valid": True})
    failed_job = {k: copy.deepcopy(v) for k, v in job_data.items() if k not in ("result_uri", "result_schema_uri")}
    failed_job.update({"state": "failed", "stage": "backend_unavailable", "error": {"code": "renderer_unavailable", "message": "The configured rendering backend is unavailable.", "retryable": True, "diagnostics": []}})
    failed_path = "examples/mcp/valid/job_get.failed.structured.json"
    write(failed_path, {"ok": True, "data": failed_job})
    cases.append({"file": failed_path, "tool": "job_get", "schema": "outputSchema", "expected_valid": True})
    missing_map = copy.deepcopy(examples["score_read_region"][1])
    missing_map["fragments"][0].pop("text_map")
    invalid = [
        ("score_import.remote-url", "score_import", "inputSchema", {"source": {"kind": "url", "url": "https://example.invalid/score.musicxml", "format": "musicxml"}, "idempotency_key": "import_demo"}),
        ("score_apply_edit.missing-idempotency", "score_apply_edit", "inputSchema", {k: v for k, v in edit.items() if k != "idempotency_key"}),
        ("score_apply_edit.caller-actor", "score_apply_edit", "inputSchema", {**edit, "actor": {"kind": "human", "principal_id": "invented"}}),
        ("score_render.missing-revision", "score_render", "inputSchema", {"score_id": "score_demo"}),
        ("result.mixed-branches", "score_get_format", "outputSchema", {**read("mcp/examples/score_get_format.structured.json"), "error": error["error"]}),
        ("result.diagnostics-overflow", "score_validate", "outputSchema", {"ok": True, "data": {**examples["score_validate"][1], "diagnostics": [{"code": "example", "message": "bounded", "severity": "info"}] * 101}}),
        ("score_read_region.missing-text-map", "score_read_region", "outputSchema", {"ok": True, "data": missing_map}),
        ("job_get.success-without-result", "job_get", "outputSchema", {"ok": True, "data": {k: v for k, v in job_data.items() if k != "result_uri"}}),
    ]
    for label, name, side, value in invalid:
        path = f"examples/mcp/invalid/{label}.json"
        write(path, value)
        cases.append({"file": path, "tool": name, "schema": side, "expected_valid": False})
    metadata = {"io.modelcontextprotocol/protocolVersion": "2026-07-28", "io.modelcontextprotocol/clientInfo": {"name": "scorestream-agent", "version": "1.0.0"}, "io.modelcontextprotocol/clientCapabilities": {}}
    wire_request = {"jsonrpc": "2.0", "id": "format-1", "method": "tools/call", "params": {"_meta": metadata, "name": "score_get_format", "arguments": {}}}
    structured = read("mcp/examples/score_get_format.structured.json")
    wire_result = {"jsonrpc": "2.0", "id": "format-1", "result": {"resultType": "complete", "content": [{"type": "text", "text": json.dumps(structured, ensure_ascii=False)}], "structuredContent": structured, "isError": False}}
    write("mcp/examples/score_get_format.request.modern.json", wire_request)
    write("mcp/examples/score_get_format.result.modern.json", wire_result)
    manifest = {"status": "design_fixtures_not_server_execution", "validation": "JSON Schema 2020-12, offline; semantic constraints require the future Rust core", "fixture_note": "Examples illustrate individual contract shapes. They are not a log of a live workspace; server-assigned identifiers, renderer names and times are sample values.", "cases": cases, "wire_examples": ["mcp/examples/score_get_format.request.modern.json", "mcp/examples/score_get_format.result.modern.json"]}
    write("examples/mcp/manifest.json", manifest)
    return manifest


def validate(docs, tools, manifest):
    registry = Registry().with_resources((uri, Resource.from_contents(schema)) for uri, schema in docs.items())
    for schema in docs.values():
        Draft202012Validator.check_schema(schema)
    by_name = {tool["name"]: tool for tool in tools}
    assert len(by_name) == 11 and list(by_name)[0] == "score_get_format"
    assert not any(any(word in name for word in ("annotation", "playback", "knowledge", "performance")) for name in by_name)

    def inspect_refs(value):
        if isinstance(value, dict):
            if "$ref" in value:
                assert value["$ref"].startswith("#/$defs/"), value["$ref"]
            for child in value.values():
                inspect_refs(child)
        elif isinstance(value, list):
            for child in value:
                inspect_refs(child)

    for tool in tools:
        for side in ("inputSchema", "outputSchema"):
            schema = tool[side]
            Draft202012Validator.check_schema(schema)
            inspect_refs(schema)
    for case in manifest["cases"]:
        schema = by_name[case["tool"]][case["schema"]]
        errors = list(Draft202012Validator(schema, format_checker=FormatChecker()).iter_errors(read(case["file"])))
        valid = not errors
        if valid != case["expected_valid"]:
            raise AssertionError(f"{case['file']}: expected valid={case['expected_valid']}; " + "; ".join(error.message for error in errors[:3]))
    # The mutation surface is exactly the shared EditRequest after bundling.
    original = Draft202012Validator(ref(EDIT, "EditRequest"), registry=registry)
    bundled = Draft202012Validator(by_name["score_apply_edit"]["inputSchema"])
    for fixture in (ROOT / "examples/auxiliary").rglob("edit-*.json"):
        value = json.loads(fixture.read_text())
        assert original.is_valid(value) == bundled.is_valid(value), fixture
    wire = read("mcp/examples/score_get_format.result.modern.json")["result"]
    assert json.loads(wire["content"][0]["text"]) == wire["structuredContent"]
    print(f"Validated {len(tools)} planned tools, 22 self-contained tool schemas, and {len(manifest['cases'])} example expectations; no server was executed.")


def main():
    build_support_schemas()
    build_domain_schemas()
    docs = {schema["$id"]: schema for path in sorted((ROOT / "schemas").glob("*.schema.json")) if "$id" in (schema := json.loads(path.read_text()))}
    tools = build_catalog(docs)
    manifest = build_examples(tools)
    validate(docs, tools, manifest)


if __name__ == "__main__":
    main()
