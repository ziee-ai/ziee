//! Tool and function calling types

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A tool that can be called by the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// The type of tool (currently only "function" is supported)
    #[serde(rename = "type")]
    pub tool_type: String,

    /// The function definition
    pub function: FunctionDefinition,
}

impl Tool {
    /// Creates a new function tool
    pub fn function(name: impl Into<String>, description: impl Into<String>, parameters: Value) -> Self {
        Self {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name: name.into(),
                description: Some(description.into()),
                parameters,
            },
        }
    }
}

/// A function definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    /// The name of the function
    pub name: String,

    /// The description of what the function does
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// The parameters the function accepts (JSON Schema format)
    pub parameters: Value,
}

/// How the model should choose which tools to call
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    /// The model can choose to call tools or not
    Auto,

    /// The model must call at least one tool
    Required,

    /// The model must call the specified tool
    Specific {
        /// The type of tool
        #[serde(rename = "type")]
        tool_type: String,
        /// The function to call
        function: ToolChoiceFunction,
    },
}

impl ToolChoice {
    /// Creates an auto tool choice
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Creates a required tool choice
    pub fn required() -> Self {
        Self::Required
    }

    /// Creates a specific function tool choice
    pub fn function(name: impl Into<String>) -> Self {
        Self::Specific {
            tool_type: "function".to_string(),
            function: ToolChoiceFunction {
                name: name.into(),
            },
        }
    }
}

/// A specific function in tool choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoiceFunction {
    /// The name of the function
    pub name: String,
}

/// A tool call made by the model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// The ID of the tool call
    pub id: String,

    /// The type of tool (currently only "function")
    #[serde(rename = "type")]
    pub tool_type: String,

    /// The function call details
    pub function: FunctionCall,
}

/// Details of a function call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// The name of the function being called
    pub name: String,

    /// The arguments to pass to the function (JSON string)
    pub arguments: String,
}

impl ToolCall {
    /// Creates a new function tool call
    pub fn function(id: impl Into<String>, name: impl Into<String>, arguments: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            tool_type: "function".to_string(),
            function: FunctionCall {
                name: name.into(),
                arguments: arguments.into(),
            },
        }
    }
}
