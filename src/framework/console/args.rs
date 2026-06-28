use std::collections::HashMap;

/// Parsed command-line arguments.
///
/// Supports:
/// - `--key=value`  named argument with value
/// - `--flag`       boolean flag (present = true)
/// - bare tokens    positional arguments (in order)
///
/// Named values use the `--key=value` form only. There is deliberately no
/// space-separated `--key value` form: without a per-command schema the parser
/// cannot tell whether the token after `--flag` is its value or the next
/// positional, so it would either swallow positionals after a boolean flag or
/// drop values that look like options (e.g. `--limit -10`). Requiring `=`
/// keeps parsing unambiguous.
pub struct Args {
    positional: Vec<String>,
    named: HashMap<String, Option<String>>,
}

impl Args {
    pub fn parse(raw: Vec<String>) -> Self {
        let mut positional = Vec::new();
        let mut named: HashMap<String, Option<String>> = HashMap::new();

        for arg in raw {
            if let Some(without_prefix) = arg.strip_prefix("--") {
                match without_prefix.split_once('=') {
                    Some((key, value)) => {
                        named.insert(key.to_string(), Some(value.to_string()));
                    }
                    None => {
                        named.insert(without_prefix.to_string(), None);
                    }
                }
            } else {
                positional.push(arg);
            }
        }

        Self { positional, named }
    }

    /// Returns the value of a named argument (`--key=value`).
    pub fn get(&self, key: &str) -> Option<&str> {
        self.named.get(key).and_then(|v| v.as_deref())
    }

    /// Returns `true` if the flag or named argument is present at all.
    pub fn has(&self, key: &str) -> bool {
        self.named.contains_key(key)
    }

    /// Returns the positional argument at the given zero-based index.
    pub fn positional(&self, index: usize) -> Option<&str> {
        self.positional.get(index).map(String::as_str)
    }
}
