use super::shell::contains_unquoted_sequence;

/// Split a command chain on `&&`, `||`, `;` outside quotes.
/// Piped commands are returned whole so rewrite can skip them safely.
/// Lines with `<<` (heredoc) or `$((` are returned whole.
pub(super) fn split_command_chain(cmd: &str) -> Vec<&str> {
    let trimmed = cmd.trim();
    if trimmed.is_empty() {
        return vec![];
    }

    if contains_unquoted_sequence(trimmed, b"<<") || contains_unquoted_sequence(trimmed, b"$((") {
        return vec![trimmed];
    }

    let mut results = Vec::new();
    let mut start = 0;
    let bytes = trimmed.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    while i < len {
        let b = bytes[i];

        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        match b {
            b'\\' if !in_single => {
                escaped = true;
                i += 1;
            }
            b'\'' if !in_double => {
                in_single = !in_single;
                i += 1;
            }
            b'"' if !in_single => {
                in_double = !in_double;
                i += 1;
            }
            b'|' if !in_single && !in_double => {
                if i + 1 < len && bytes[i + 1] == b'|' {
                    let segment = trimmed[start..i].trim();
                    if !segment.is_empty() {
                        results.push(segment);
                    }
                    i += 2;
                    start = i;
                } else {
                    return vec![trimmed];
                }
            }
            b'&' if !in_single && !in_double && i + 1 < len && bytes[i + 1] == b'&' => {
                let segment = trimmed[start..i].trim();
                if !segment.is_empty() {
                    results.push(segment);
                }
                i += 2;
                start = i;
            }
            b';' if !in_single && !in_double => {
                let segment = trimmed[start..i].trim();
                if !segment.is_empty() {
                    results.push(segment);
                }
                i += 1;
                start = i;
            }
            _ => {
                i += 1;
            }
        }
    }

    if start < len {
        let segment = trimmed[start..].trim();
        if !segment.is_empty() {
            results.push(segment);
        }
    }

    results
}
