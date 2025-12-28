use super::ToolCall;
use serde_json::Value;

pub struct ToolCallParser;

impl ToolCallParser {
    pub fn extract_from_content(content: &str) -> Vec<ToolCall> {
        // 尝试 JSON 格式解析
        if let Ok(calls) = Self::parse_json_format(content) {
            if !calls.is_empty() {
                return calls;
            }
        }
        
        // 尝试 XML 格式解析
        Self::parse_xml_format(content)
    }

    pub fn parse_json_format(content: &str) -> Result<Vec<ToolCall>, serde_json::Error> {
        // 查找 JSON 对象
        if let Some(start) = content.find('{') {
            if let Some(end) = content.rfind('}') {
                let json_str = &content[start..=end];
                if let Ok(json) = serde_json::from_str::<Value>(json_str) {
                    if let Some(tool_calls) = json.get("tool_calls").and_then(|v| v.as_array()) {
                        let mut calls = Vec::new();
                        for (i, call) in tool_calls.iter().enumerate() {
                            if let (Some(name), Some(params)) = (
                                call.get("name").and_then(|v| v.as_str()),
                                call.get("parameters")
                            ) {
                                calls.push(ToolCall {
                                    id: call.get("id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or(&format!("call_{}", i))
                                        .to_string(),
                                    name: name.to_string(),
                                    parameters: params.clone(),
                                });
                            }
                        }
                        return Ok(calls);
                    }
                }
            }
        }
        Ok(Vec::new())
    }

    pub fn parse_xml_format(content: &str) -> Vec<ToolCall> {
        let mut calls = Vec::new();
        let mut current_pos = 0;
        
        while let Some(start) = content[current_pos..].find("<tool_call") {
            let start_pos = current_pos + start;
            if let Some(end) = content[start_pos..].find("</tool_call>") {
                let end_pos = start_pos + end + "</tool_call>".len();
                let xml_block = &content[start_pos..end_pos];
                
                if let Some(call) = Self::parse_single_xml_call(xml_block) {
                    calls.push(call);
                }
                
                current_pos = end_pos;
            } else {
                break;
            }
        }
        
        calls
    }

    fn parse_single_xml_call(xml: &str) -> Option<ToolCall> {
        // 简单的 XML 解析
        let id = Self::extract_xml_attribute(xml, "id").unwrap_or_else(|| "call_0".to_string());
        let name = Self::extract_xml_attribute(xml, "name")?;
        
        // 解析参数
        let mut parameters = serde_json::Map::new();
        if let Some(params_start) = xml.find("<parameters>") {
            if let Some(params_end) = xml.find("</parameters>") {
                let params_content = &xml[params_start + "<parameters>".len()..params_end];
                parameters = Self::parse_xml_parameters(params_content);
            }
        }
        
        Some(ToolCall {
            id,
            name,
            parameters: Value::Object(parameters),
        })
    }

    fn extract_xml_attribute(xml: &str, attr: &str) -> Option<String> {
        let pattern = format!("{}=\"", attr);
        if let Some(start) = xml.find(&pattern) {
            let value_start = start + pattern.len();
            if let Some(end) = xml[value_start..].find('"') {
                return Some(xml[value_start..value_start + end].to_string());
            }
        }
        None
    }

    fn parse_xml_parameters(content: &str) -> serde_json::Map<String, Value> {
        let mut params = serde_json::Map::new();
        let mut current_pos = 0;
        
        while let Some(start) = content[current_pos..].find('<') {
            let tag_start = current_pos + start;
            if let Some(tag_end) = content[tag_start..].find('>') {
                let tag_end_pos = tag_start + tag_end;
                let tag_name = &content[tag_start + 1..tag_end_pos];
                
                if !tag_name.starts_with('/') {
                    let close_tag = format!("</{}>", tag_name);
                    if let Some(close_pos) = content[tag_end_pos..].find(&close_tag) {
                        let value_start = tag_end_pos + 1;
                        let value_end = tag_end_pos + close_pos;
                        let value = content[value_start..value_end].trim();
                        
                        // 尝试解析为数字
                        if let Ok(num) = value.parse::<f64>() {
                            params.insert(tag_name.to_string(), Value::Number(serde_json::Number::from_f64(num).unwrap()));
                        } else {
                            params.insert(tag_name.to_string(), Value::String(value.to_string()));
                        }
                        
                        current_pos = value_end + close_tag.len();
                    } else {
                        break;
                    }
                } else {
                    current_pos = tag_end_pos + 1;
                }
            } else {
                break;
            }
        }
        
        params
    }
}
