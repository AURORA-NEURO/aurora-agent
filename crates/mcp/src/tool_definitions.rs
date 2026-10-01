use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

static TOOL_DEFINITIONS: OnceLock<Vec<Value>> = OnceLock::new();
static TOOL_DEFINITIONS_BY_NAME: OnceLock<HashMap<&'static str, &'static Value>> = OnceLock::new();

fn parse_embedded_tool_definitions() -> Vec<Value> {
    serde_json::from_str(include_str!("tool_definitions.json"))
        .expect("embedded MCP tool catalogue must be valid JSON")
}

fn definitions() -> &'static [Value] {
    TOOL_DEFINITIONS
        .get_or_init(parse_embedded_tool_definitions)
        .as_slice()
}

/// Return the immutable embedded catalogue as an owned value for MCP `tools/list`.
pub fn tool_definitions() -> Vec<Value> {
    definitions().to_vec()
}

pub(crate) fn find_tool_definition(name: &str) -> Option<&'static Value> {
    TOOL_DEFINITIONS_BY_NAME
        .get_or_init(|| {
            let definitions = definitions();
            let mut index = HashMap::with_capacity(definitions.len());
            for definition in definitions {
                let name = definition
                    .get("name")
                    .and_then(Value::as_str)
                    .expect("every embedded MCP tool definition must have a name");
                assert!(
                    index.insert(name, definition).is_none(),
                    "embedded MCP tool names must be unique"
                );
            }
            index
        })
        .get(name)
        .copied()
}

#[cfg(test)]
mod tests {
    use super::parse_embedded_tool_definitions;
    use serde_json::Value;

    #[test]
    fn embedded_tool_catalogue_parses_on_small_stack_with_exact_catalogue_digest() {
        let definitions = std::thread::Builder::new()
            .stack_size(2 * 1024 * 1024)
            .spawn(parse_embedded_tool_definitions)
            .expect("small-stack catalogue test thread starts")
            .join()
            .expect("embedded catalogue parses without a large activation record");
        assert_eq!(definitions.len(), 983);
        assert_eq!(
            bioprism_ids::ContentHash::of_value(&Value::Array(definitions))
                .expect("catalogue canonicalizes")
                .to_string(),
            "44e8753e41159636ba046d52879cf640b67be204231cd431c702432b5fed3b51"
        );
    }

    #[test]
    fn catalogue_name_index_resolves_every_tool_and_refuses_unknown_names() {
        for definition in super::definitions() {
            let name = definition["name"]
                .as_str()
                .expect("embedded definition has a string name");
            let indexed = super::find_tool_definition(name)
                .unwrap_or_else(|| panic!("catalogue index omitted {name}"));
            assert!(std::ptr::eq(indexed, definition));
        }
        assert!(super::find_tool_definition("not-a-real-mcp-tool").is_none());
    }
}
