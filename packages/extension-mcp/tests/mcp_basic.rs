use aa_kernel::tool_pack::ToolPack;

#[test]
fn test_from_json_empty() {
    let pack = aa_extension_mcp::McpToolPack::from_json(serde_json::json!({}));
    let scope = aa_kernel::ToolPackScope::new(".");
    let tools = pack.tools(&scope);
    assert!(tools.is_empty(), "no servers should mean no tools");
}

#[test]
fn test_from_json_parsing() {
    let json = serde_json::json!({
        "weather": {
            "command": "python3",
            "args": ["-m", "mcp_weather_server"]
        }
    });

    let pack = aa_extension_mcp::McpToolPack::from_json(json);
    let scope = aa_kernel::ToolPackScope::new(".");
    let tools = pack.tools(&scope);
    assert!(tools.is_empty());
}
