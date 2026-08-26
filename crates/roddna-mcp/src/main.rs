//! caneDNA MCP server — exposes the design engine over the Model Context
//! Protocol so an assistant can turn plain language into a taper.
//!
//! Transport: MCP stdio — newline-delimited JSON-RPC 2.0 on stdin/stdout, logs
//! on stderr. We implement the handshake by hand (no async runtime, no SDK) to
//! keep the dependency footprint the same as the rest of the workspace:
//! `serde_json` + `roddna-core`.
//!
//! Tools:
//!
//! `design_rod` — a spec (line weight, length, pieces, action, optional seed
//! filter) becomes an adapted taper plus rationale (the stage-D engine).
//!
//! `list_tapers` — browse/search the embedded library so the assistant can
//! ground line-weight/action choices and pick a seed.
//!
//! The library is embedded at compile time, same as the GUI, so the binary is
//! self-contained — no runtime data files.

use std::io::{BufRead, Write};

use roddna_core::{ActionClass, DesignRequest, Library, ModalParams, Taper};
use serde_json::{json, Value};

const TAPERS_JSON: &str = include_str!("../../../data/tapers.json");
const PROTOCOL_VERSION: &str = "2024-11-05";

fn main() {
    let library = Library::from_json(TAPERS_JSON).expect("bundled tapers.json is valid");
    let modal = ModalParams::default();

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("roddna-mcp: stdin read error: {e}");
                break;
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("roddna-mcp: bad JSON: {e}");
                continue;
            }
        };

        // Notifications carry no `id` and never get a response.
        let id = msg.get("id").cloned();
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");

        let response = handle(method, &msg, id.clone(), &library, &modal);
        if let Some(resp) = response {
            if let Err(e) = writeln!(out, "{resp}").and_then(|()| out.flush()) {
                eprintln!("roddna-mcp: stdout write error: {e}");
                break;
            }
        }
    }
}

/// Route one JSON-RPC message. Returns `Some(response)` for requests, `None`
/// for notifications (no `id`) which the protocol says must not be answered.
fn handle(
    method: &str,
    msg: &Value,
    id: Option<Value>,
    library: &Library,
    modal: &ModalParams,
) -> Option<Value> {
    let id = id?; // notification — nothing to send back

    match method {
        "initialize" => Some(ok(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "caneDNA", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "caneDNA design engine. Use list_tapers to browse the \
                    library (873 attributed bamboo fly-rod tapers) and ground your line \
                    weight / action choices, then design_rod to adapt the closest seed \
                    into a new taper. For a spey request, pass seed_contains=\"spey\" so \
                    only spey tapers are used as seeds."
            }),
        )),
        "tools/list" => Some(ok(id, json!({ "tools": tool_specs() }))),
        "tools/call" => Some(tools_call(id, msg, library, modal)),
        "ping" => Some(ok(id, json!({}))),
        other => Some(err(
            id,
            -32601,
            &format!("method not found: {other}"),
        )),
    }
}

fn tool_specs() -> Value {
    json!([
        {
            "name": "design_rod",
            "description": "Design a split-bamboo fly-rod taper from a plain-language spec. \
                Picks the closest-fitting library taper as a seed and adapts it (rescales to \
                the target length, sets line weight / pieces), returning the full station-by-\
                station taper, its achieved action, and a rationale. Parse the user's request \
                into these fields; for a trout-spey / two-handed request set seed_contains to \
                \"spey\" so only spey tapers seed it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "line_weight": {
                        "type": "number",
                        "description": "AFTM line weight the rod should cast, e.g. 5. For trout spey, the single-hand-equivalent line weight (commonly 2-5)."
                    },
                    "length_in": {
                        "type": "number",
                        "description": "Overall length in INCHES. 7'6\" = 90, an 11'0\" trout spey = 132."
                    },
                    "pieces": {
                        "type": "number",
                        "description": "Number of sections. Default 2.",
                        "default": 2
                    },
                    "action": {
                        "type": "string",
                        "enum": ["fast", "medium", "full"],
                        "description": "Action feel. fast = tip action (streamers, distance); medium = progressive; full = full-flex / parabolic (delicate dry-fly and wet-fly presentation). 'Dries and wets, not streamers' => full (or medium)."
                    },
                    "seed_contains": {
                        "type": "string",
                        "description": "Optional. Restrict seed selection to library rods whose name contains this text (case-insensitive), e.g. \"spey\" or \"Payne\". Use for 'like the X rods' or spey requests."
                    },
                    "seed_name": {
                        "type": "string",
                        "description": "Optional. Force a specific seed by exact library name (see list_tapers)."
                    }
                },
                "required": ["line_weight", "length_in", "action"]
            }
        },
        {
            "name": "list_tapers",
            "description": "Browse or search the embedded caneDNA library of attributed bamboo \
                fly-rod tapers. Use this first to see what seeds exist, confirm a maker/model name, \
                or ground a line-weight or action choice before calling design_rod.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "contains": {
                        "type": "string",
                        "description": "Case-insensitive substring to filter rod names, e.g. \"spey\", \"Cattanach\", \"7'6\"."
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max rows to return. Default 40.",
                        "default": 40
                    }
                }
            }
        }
    ])
}

fn tools_call(id: Value, msg: &Value, library: &Library, modal: &ModalParams) -> Value {
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    let result = match name {
        "design_rod" => design_rod(&args, library, modal),
        "list_tapers" => list_tapers(&args, library, modal),
        other => Err(format!("unknown tool: {other}")),
    };

    match result {
        Ok(text) => ok(
            id,
            json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
        ),
        // Tool-level failures are reported as a result with isError=true (per the
        // MCP spec) so the model sees them, not as a JSON-RPC protocol error.
        Err(e) => ok(
            id,
            json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
        ),
    }
}

fn design_rod(args: &Value, library: &Library, modal: &ModalParams) -> Result<String, String> {
    let line_weight = num(args, "line_weight").ok_or("line_weight is required (a number)")?;
    let length_in = num(args, "length_in").ok_or("length_in is required (inches, e.g. 90)")?;
    let pieces = num(args, "pieces").unwrap_or(2.0);
    let action = args
        .get("action")
        .and_then(Value::as_str)
        .ok_or("action is required (fast | medium | full)")?;
    let action = parse_action(action)?;

    // Optional seed narrowing — build a filtered view of the library so the
    // stage-D scorer only considers matching rods. This is how a "like the spey
    // rods" request stays honest without changing the core DesignRequest.
    let seed_contains = args.get("seed_contains").and_then(Value::as_str);
    let seed_name = args.get("seed_name").and_then(Value::as_str);

    let mut view = library.clone();
    if let Some(exact) = seed_name {
        view.models.retain(|t| t.name.as_deref() == Some(exact));
        if view.models.is_empty() {
            return Err(format!(
                "no library rod is named exactly \"{exact}\" — use list_tapers to find the right name"
            ));
        }
    } else if let Some(sub) = seed_contains {
        let needle = sub.to_lowercase();
        view.models
            .retain(|t| t.name.as_deref().unwrap_or("").to_lowercase().contains(&needle));
        if view.models.is_empty() {
            return Err(format!(
                "no library rod name contains \"{sub}\" — use list_tapers to browse what's available"
            ));
        }
    }

    let req = DesignRequest {
        line_weight,
        length_in,
        pieces,
        action,
    };

    let result = view.design(&req, modal).ok_or_else(|| {
        "no eligible seed had the stress inputs needed to design against this spec".to_string()
    })?;

    let mut s = String::new();
    s.push_str(&result.rationale);
    s.push_str("\n\nTaper (station inches from tip \u{2192} flat-to-flat dimension, inches):\n");
    let t = &result.taper;
    for (st, dim) in t.stations.iter().zip(t.dimensions.iter()) {
        s.push_str(&format!("  {st:>6.1}  {dim:.4}\n"));
    }
    let scope = seed_name
        .map(|n| format!("\nSeed restricted to: {n}"))
        .or_else(|| seed_contains.map(|c| format!("\nSeeds restricted to names containing \"{c}\".")))
        .unwrap_or_default();
    s.push_str(&scope);
    s.push_str("\n\nOpen this taper in caneDNA's design mode to refine it (per-station edits, solve-to-flat-stress).");
    Ok(s)
}

fn list_tapers(args: &Value, library: &Library, modal: &ModalParams) -> Result<String, String> {
    let contains = args
        .get("contains")
        .and_then(Value::as_str)
        .map(|s| s.to_lowercase());
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(40)
        .max(1) as usize;

    let matches: Vec<&Taper> = library
        .models
        .iter()
        .filter(|t| match &contains {
            Some(sub) => t.name.as_deref().unwrap_or("").to_lowercase().contains(sub),
            None => true,
        })
        .collect();

    let total = matches.len();
    let mut s = format!(
        "{total} of {} library tapers match{}.\n\n",
        library.models.len(),
        contains
            .as_ref()
            .map(|c| format!(" \"{c}\""))
            .unwrap_or_default()
    );
    for t in matches.iter().take(limit) {
        let name = t.name.as_deref().unwrap_or("(unnamed)");
        let lw = t
            .line_weight
            .map(|w| format!("{w:.0}-wt"))
            .unwrap_or_else(|| "?-wt".into());
        let len = t
            .length
            .or_else(|| t.stations.last().copied())
            .map(fmt_ft)
            .unwrap_or_else(|| "?".into());
        let action = t
            .action_profile(modal)
            .map(|a| a.class.label())
            .unwrap_or("(no physics)");
        s.push_str(&format!("  {name}  \u{2014}  {lw}, {len}, {action}\n"));
    }
    if total > limit {
        s.push_str(&format!(
            "\n\u{2026} {} more not shown; narrow with `contains` or raise `limit`.",
            total - limit
        ));
    }
    Ok(s)
}

fn parse_action(s: &str) -> Result<ActionClass, String> {
    match s.trim().to_lowercase().as_str() {
        "fast" | "tip" => Ok(ActionClass::Fast),
        "medium" | "moderate" | "progressive" => Ok(ActionClass::Medium),
        "full" | "full-flex" | "slow" | "parabolic" => Ok(ActionClass::Full),
        other => Err(format!(
            "unknown action \"{other}\" — use fast, medium, or full"
        )),
    }
}

fn num(args: &Value, key: &str) -> Option<f64> {
    args.get(key).and_then(Value::as_f64)
}

/// Inches to a feet+inches label, e.g. 90.0 -> "7'6\"".
fn fmt_ft(inches: f64) -> String {
    let ft = (inches / 12.0).floor() as i64;
    let rem = inches - (ft as f64) * 12.0;
    if rem.abs() < 0.05 {
        format!("{ft}'")
    } else {
        format!("{ft}'{rem:.0}\"")
    }
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn err(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}
