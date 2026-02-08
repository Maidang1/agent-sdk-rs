use agent_sdk::tool::ToolCallParser;

fn main() {
    // 测试 JSON 格式解析
    let json_content = r#"
    I need to calculate 15 * 23. Let me use the calculator tool.
    
    {
      "tool_calls": [
        {
          "id": "call_1",
          "name": "calculator",
          "parameters": {
            "a": 15,
            "b": 23,
            "operation": "mul"
          }
        }
      ]
    }
    "#;
    
    let calls = ToolCallParser::extract_from_content(json_content);
    println!("Extracted {} tool calls from JSON", calls.len());
    for call in &calls {
        println!("Call: {} -> {} with params: {}", call.id, call.name, call.parameters);
    }
    
    // 测试 XML 格式解析
    let xml_content = r#"
    I need to calculate 15 * 23. Let me use the calculator tool.
    
    <tool_call id="call_1" name="calculator">
      <parameters>
        <a>15</a>
        <b>23</b>
        <operation>mul</operation>
      </parameters>
    </tool_call>
    "#;
    
    let calls = ToolCallParser::extract_from_content(xml_content);
    println!("Extracted {} tool calls from XML", calls.len());
    for call in &calls {
        println!("Call: {} -> {} with params: {}", call.id, call.name, call.parameters);
    }
}
