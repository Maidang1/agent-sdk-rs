use crate::provider::GenerateOptions;

#[derive(Debug, Clone)]
pub struct AgentOptions {
    pub system_prompt: Option<String>,
    pub max_iterations: usize,
    pub tool_choice: ToolChoice,
    pub generate_options: GenerateOptions,
}

impl Default for AgentOptions {
    fn default() -> Self {
        Self {
            system_prompt: None,
            max_iterations: 10,
            tool_choice: ToolChoice::Auto,
            generate_options: GenerateOptions::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ToolChoice {
    Auto,
    None,
    Required,
    Specific(String),
}
