use aa_kernel::tool_provider::ToolProvider;

#[test]
fn test_from_json_empty() {
    let provider = aa_extension_mcp::McpToolProvider::from_json(serde_json::json!({}));
    let scope = aa_kernel::ToolProviderScope::new(".");
    let tools = provider.tools(&scope);
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

    let provider = aa_extension_mcp::McpToolProvider::from_json(json);
    let scope = aa_kernel::ToolProviderScope::new(".");
    let tools = provider.tools(&scope);
    assert!(tools.is_empty());
}
