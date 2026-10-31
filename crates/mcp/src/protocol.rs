//! JSON-RPC 2.0 and the MCP methods, as a pure function of one message.

use serde_json::{json, Value};

/// Protocol versions spoken, newest first.
pub const PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;

/// A tool the game offers.
#[derive(Clone, Debug, PartialEq)]
pub struct Tool {
    pub name: String,
    pub description: String,
    /// JSON Schema of the arguments (an object schema).
    pub input_schema: Value,
}

/// A tool's answer: structured data, or an error the agent should read.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolResult {
    Ok(Value),
    Err(String),
}

pub fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// Why a call's arguments do not fit its tool's schema: not an object, or
/// naming an argument the schema does not list (with the ones it does).
/// None when they fit. A misspelt name was ignored without a word (an
/// agent's `distance` for `set_camera`'s `distance_m`, 2026-10-31).
/// Missing required arguments are each tool's to explain.
pub fn unknown_arguments(schema: &Value, args: &Value) -> Option<String> {
    let Some(args) = args.as_object() else { return Some("the arguments must be an object".into()) };
    let known = schema.get("properties").and_then(Value::as_object)?;
    let unknown: Vec<String> = args.keys().filter(|k| !known.contains_key(*k)).map(|k| format!("`{k}`")).collect();
    if unknown.is_empty() {
        return None;
    }
    let takes: Vec<&str> = known.keys().map(String::as_str).collect();
    let takes = if takes.is_empty() { "no arguments".to_string() } else { takes.join(", ") };
    Some(format!("unknown argument {} (this tool takes: {takes})", unknown.join(", ")))
}

fn result(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

/// Handles one JSON-RPC message; `None` for notifications and responses.
/// `call` runs a tool (in the game) and returns its result.
pub fn handle(message: Value, tools: &[Tool], mut call: impl FnMut(&str, Value) -> ToolResult) -> Option<Value> {
    let Some(obj) = message.as_object() else {
        return Some(error(Value::Null, INVALID_REQUEST, "not an object"));
    };
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        // A response from the client (we send no requests): ignore.
        return None;
    };
    let id = obj.get("id").cloned()?; // a notification: no answer
    let params = obj.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str);
            let version = asked.filter(|v| PROTOCOL_VERSIONS.contains(v)).unwrap_or(PROTOCOL_VERSIONS[0]);
            result(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "sunscatter", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": "Sunscatter, a real-scale spaceflight game. You act from a control \
                        location (a crewed vessel or mission control) like the player: what you see of \
                        other vessels and your commands to them travel at light speed."
                }),
            )
        }
        "ping" => result(id, json!({})),
        "tools/list" => {
            let list: Vec<Value> = tools
                .iter()
                .map(|t| json!({"name": t.name, "description": t.description, "inputSchema": t.input_schema}))
                .collect();
            result(id, json!({"tools": list}))
        }
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return Some(error(id, INVALID_PARAMS, "missing tool name"));
            };
            let Some(tool) = tools.iter().find(|t| t.name == name) else {
                return Some(error(id, INVALID_PARAMS, &format!("unknown tool {name}")));
            };
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            let answer = match unknown_arguments(&tool.input_schema, &args) {
                Some(e) => ToolResult::Err(e),
                None => call(name, args),
            };
            match answer {
                ToolResult::Ok(v) => result(
                    id,
                    json!({"content": [{"type": "text", "text": v.to_string()}], "structuredContent": v, "isError": false}),
                ),
                ToolResult::Err(e) => result(id, json!({"content": [{"type": "text", "text": e}], "isError": true})),
            }
        }
        _ => error(id, METHOD_NOT_FOUND, &format!("unknown method {method}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Vec<Tool> {
        vec![Tool { name: "get_state".into(), description: "d".into(), input_schema: json!({"type": "object"}) }]
    }

    fn no_call(_: &str, _: Value) -> ToolResult {
        panic!("no tool call expected")
    }

    #[test]
    fn initialize_negotiates_the_version() {
        let m = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26"}});
        let r = handle(m, &tools(), no_call).unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2025-03-26");
        assert!(r["result"]["capabilities"]["tools"].is_object());
        let m = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1999-01-01"}});
        assert_eq!(handle(m, &tools(), no_call).unwrap()["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
    }

    #[test]
    fn notifications_and_responses_get_no_answer() {
        assert!(handle(json!({"jsonrpc":"2.0","method":"notifications/initialized"}), &tools(), no_call).is_none());
        assert!(handle(json!({"jsonrpc":"2.0","id":5,"result":{}}), &tools(), no_call).is_none());
    }

    #[test]
    fn tools_are_listed_and_called() {
        let r = handle(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}), &tools(), no_call).unwrap();
        assert_eq!(r["result"]["tools"][0]["name"], "get_state");
        assert!(r["result"]["tools"][0]["inputSchema"].is_object());
        let m = json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_state","arguments":{}}});
        let r = handle(m, &tools(), |name, _| ToolResult::Ok(json!({"tool": name}))).unwrap();
        assert_eq!(r["result"]["structuredContent"]["tool"], "get_state");
        assert_eq!(r["result"]["isError"], false);
        let m = json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"get_state"}});
        let r = handle(m, &tools(), |_, _| ToolResult::Err("no signal".into())).unwrap();
        assert_eq!(r["result"]["isError"], true);
    }

    #[test]
    fn arguments_the_schema_does_not_name_are_refused() {
        let schema = json!({"type": "object", "properties": {"distance_m": {}, "pitch_deg": {}}, "required": []});
        let none = json!({"type": "object", "properties": {}});
        // (schema, arguments, refused, words in the answer)
        let cases = [
            (&schema, json!({}), false, vec![]),
            (&schema, json!({"distance_m": 40, "pitch_deg": -15}), false, vec![]),
            (&schema, json!({"distance": 40}), true, vec!["`distance`", "distance_m, pitch_deg"]),
            (&schema, json!({"distance": 40, "yaw": 1}), true, vec!["`distance`, `yaw`"]),
            (&schema, json!([40]), true, vec!["object"]),
            (&none, json!({"x": 1}), true, vec!["no arguments"]),
            (&json!({"type": "object"}), json!({"x": 1}), false, vec![]), // a schema listing no properties
        ];
        for (schema, args, refused, words) in cases {
            let got = unknown_arguments(schema, &args);
            assert_eq!(got.is_some(), refused, "{args}: {got:?}");
            for w in words {
                assert!(got.as_deref().unwrap_or("").contains(w), "{args}: {got:?} lacks {w}");
            }
        }
        // Through the protocol: a tool error the agent reads, the tool not called.
        let tools = vec![Tool { name: "set_camera".into(), description: "d".into(), input_schema: schema.clone() }];
        let m = json!({"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"set_camera","arguments":{"distance":40}}});
        let r = handle(m, &tools, no_call).unwrap();
        assert_eq!(r["result"]["isError"], true);
    }

    #[test]
    fn unknown_things_are_errors() {
        let r = handle(json!({"jsonrpc":"2.0","id":6,"method":"resources/list"}), &tools(), no_call).unwrap();
        assert_eq!(r["error"]["code"], METHOD_NOT_FOUND);
        let m = json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"launch_nukes"}});
        assert_eq!(handle(m, &tools(), no_call).unwrap()["error"]["code"], INVALID_PARAMS);
        assert_eq!(handle(json!([1]), &tools(), no_call).unwrap()["error"]["code"], INVALID_REQUEST);
    }
}
