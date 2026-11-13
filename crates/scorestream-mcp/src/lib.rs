//! Read-only MCP format discovery. No score-edit tool is advertised until it
//! can connect to the same exclusive workspace authority as the desktop.
use serde_json::{Value, json};
const REFERENCE: &str = include_str!("../../../design/spec/agent-reference.md");
const GRAMMAR: &str = include_str!("../../../design/spec/scorestream.ebnf");
const LANGUAGE: &str = include_str!("../../../design/spec/scorestream-language.json");
const AST: &str = include_str!("../../../design/schemas/scorestream-ast.schema.json");
const INPUT: &str = include_str!("../../../design/schemas/mcp/score_get_format.input.schema.json");
const FULL: &str =
    include_str!("../../../design/examples/mcp/valid/score_get_format.full.structured.json");
const TOPICS: &[&str] = &[
    "syntax",
    "pitch",
    "time",
    "polyphony",
    "state",
    "marks",
    "relations",
    "editing",
    "extensions",
    "validation",
];
fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
fn tool_error(message: &str) -> Value {
    json!({"content":[{"type":"text","text":message}],"isError":true})
}
fn format_result(args: &Value) -> Value {
    if !args.is_object() {
        return tool_error("Arguments must be an object");
    }
    for (key, value) in args.as_object().unwrap() {
        let valid = match key.as_str() {
            "format_version" => value == "1.1",
            "profile" => value == "core-measured",
            "detail" => value == "quick" || value == "full",
            "topics" => value.as_array().is_some_and(|a| {
                a.len() <= 10
                    && a.iter()
                        .all(|v| v.as_str().is_some_and(|s| TOPICS.contains(&s)))
                    && a.iter().enumerate().all(|(i, v)| !a[..i].contains(v))
            }),
            _ => false,
        };
        if !valid {
            return tool_error(&format!("Unsupported or invalid argument: {key}"));
        }
    }
    let mut data: Value = serde_json::from_str(FULL).unwrap();
    data["data"]["status"] = json!(
        "format discovery implemented; desktop/core initial milestone; operational MCP score tools unavailable"
    );
    let full = args["detail"] == "full" || args.get("topics").is_some();
    let text = if full {
        format!(
            "{}\n\nFull machine-readable format contract:\n{}",
            REFERENCE,
            serde_json::to_string_pretty(&data["data"]).unwrap()
        )
    } else {
        REFERENCE.into()
    };
    json!({"content":[{"type":"text","text":text}],"structuredContent":data,"isError":false})
}
fn resources() -> Value {
    json!({"resources":[
        {"uri":"scorestream://spec/1.1/agent-reference","name":"ScoreStream authoring reference","mimeType":"text/markdown"},
        {"uri":"scorestream://spec/1.1/grammar","name":"ScoreStream EBNF","mimeType":"text/plain"},
        {"uri":"scorestream://spec/1.1/language","name":"ScoreStream language contract","mimeType":"application/json"},
        {"uri":"scorestream://spec/1.1/ast-schema","name":"ScoreStream AST schema","mimeType":"application/schema+json"},
        {"uri":"scorestream://capabilities","name":"Implemented capabilities","mimeType":"application/json"}]})
}
fn read_resource(uri: &str) -> Option<Value> {
    let (mime,text)=match uri {
        "scorestream://spec/1.1/agent-reference"=>("text/markdown",REFERENCE.to_string()),
        "scorestream://spec/1.1/grammar"=>("text/plain",GRAMMAR.to_string()),
        "scorestream://spec/1.1/language"=>("application/json",LANGUAGE.to_string()),
        "scorestream://spec/1.1/ast-schema"=>("application/schema+json",AST.to_string()),
        "scorestream://capabilities"=>("application/json",json!({"format_version":"1.1","mcp_tools":["score_get_format"],"workspace_access":false,"score_edits":false,"musicxml":false,"rendering":false,"playback":false,"annotations":false}).to_string()),
        _=>return None,
    };
    Some(json!({"contents":[{"uri":uri,"mimeType":mime,"text":text}]}))
}
pub fn handle(request: Value) -> Option<Value> {
    let id = request.get("id").cloned();
    if request["jsonrpc"] != "2.0" || !request["method"].is_string() {
        return Some(error(id.unwrap_or(Value::Null), -32600, "Invalid Request"));
    }
    let method = request["method"].as_str().unwrap();
    let id = id?;
    let params = &request["params"];
    let result = match method {
        "initialize" => {
            let requested = params["protocolVersion"].as_str().unwrap_or("2025-11-25");
            let version =
                if ["2024-11-05", "2025-03-26", "2025-06-18", "2025-11-25"].contains(&requested) {
                    requested
                } else {
                    "2025-11-25"
                };
            json!({"protocolVersion":version,"capabilities":{"tools":{"listChanged":false},"resources":{"subscribe":false,"listChanged":false}},"serverInfo":{"name":"scorestream-format","version":"0.1.0"},"instructions":"Read-only ScoreStream 1.1 language discovery. This process does not open or edit a score workspace."})
        }
        "ping" => json!({}),
        "tools/list" => {
            json!({"tools":[{"name":"score_get_format","description":"Learn the ScoreStream 1.1 notation syntax, exact-time rules, and examples.","inputSchema":serde_json::from_str::<Value>(INPUT).unwrap(),"annotations":{"readOnlyHint":true,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}}]})
        }
        "tools/call" => {
            if params["name"] != "score_get_format" {
                return Some(error(
                    id,
                    -32602,
                    "Unknown tool; only score_get_format is implemented",
                ));
            }
            format_result(params.get("arguments").unwrap_or(&json!({})))
        }
        "resources/list" => resources(),
        "resources/templates/list" => json!({"resourceTemplates":[]}),
        "resources/read" => match read_resource(params["uri"].as_str().unwrap_or("")) {
            Some(v) => v,
            None => return Some(error(id, -32002, "Resource not found")),
        },
        _ => return Some(error(id, -32601, "Method not found")),
    };
    Some(json!({"jsonrpc":"2.0","id":id,"result":result}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_only() {
        let r = handle(json!({"jsonrpc":"2.0","id":1,"method":"tools/list"})).unwrap();
        assert_eq!(r["result"]["tools"].as_array().unwrap().len(), 1);
        assert_eq!(r["result"]["tools"][0]["name"], "score_get_format");
    }
    #[test]
    fn defaults_and_invalid_args() {
        assert_eq!(format_result(&json!({}))["isError"], false);
        assert_eq!(
            format_result(&json!({"profile":"invented"}))["isError"],
            true
        );
        assert_eq!(
            format_result(&json!({"topics":["time","time"]}))["isError"],
            true
        );
        assert_eq!(format_result(&json!({"unknown":true}))["isError"], true);
    }
    #[test]
    fn resources_are_local_and_finite() {
        assert!(read_resource("scorestream://spec/1.1/grammar").is_some());
        assert!(read_resource("file:///etc/passwd").is_none());
    }
    #[test]
    fn notifications_are_silent() {
        assert!(handle(json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
    }
    #[test]
    fn no_unimplemented_mutation() {
        let r=handle(json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"score_apply_edit"}})).unwrap();
        assert_eq!(r["error"]["code"], -32602);
    }
}
