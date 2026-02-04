//! Simple tokenizer for console commands.
//!
//! Parses space-separated tokens with support for quoted strings.
//! No external dependencies.

/// Result of tokenizing a command string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenizedCommand<'a> {
    /// The command name (first token).
    pub command: &'a str,
    /// The arguments (remaining tokens).
    pub args: Vec<&'a str>,
    /// The raw input string.
    pub raw: &'a str,
}

/// Tokenize error types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenizeError {
    /// Empty input string.
    EmptyInput,
    /// Unterminated quoted string.
    UnterminatedString { position: usize },
}

impl std::fmt::Display for TokenizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenizeError::EmptyInput => write!(f, "empty input"),
            TokenizeError::UnterminatedString { position } => {
                write!(f, "unterminated string at position {}", position)
            }
        }
    }
}

impl std::error::Error for TokenizeError {}

/// Tokenize a command string into command name and arguments.
///
/// # Syntax
///
/// - Tokens are separated by whitespace
/// - Quoted strings (single or double) preserve whitespace
/// - Escape sequences: `\"`, `\'`, `\\`
/// - Comments: `//` to end of line (only at token boundaries)
///
/// # Examples
///
/// ```
/// use bevy_console::core::tokenize;
///
/// // Simple command
/// let result = tokenize("echo hello world").unwrap();
/// assert_eq!(result.command, "echo");
/// assert_eq!(result.args, vec!["hello", "world"]);
///
/// // Quoted strings
/// let result = tokenize(r#"say "hello world""#).unwrap();
/// assert_eq!(result.command, "say");
/// assert_eq!(result.args, vec!["hello world"]);
///
/// // Mixed
/// let result = tokenize(r#"bind F1 "toggle sv_cheats""#).unwrap();
/// assert_eq!(result.command, "bind");
/// assert_eq!(result.args, vec!["F1", "toggle sv_cheats"]);
/// ```
pub fn tokenize(input: &str) -> Result<TokenizedCommand<'_>, TokenizeError> {
    let trimmed = input.trim();

    // Handle comments - strip everything after //
    let trimmed = if let Some(comment_pos) = trimmed.find("//") {
        trimmed[..comment_pos].trim()
    } else {
        trimmed
    };

    if trimmed.is_empty() {
        return Err(TokenizeError::EmptyInput);
    }

    let tokens = tokenize_string(trimmed)?;

    if tokens.is_empty() {
        return Err(TokenizeError::EmptyInput);
    }

    let command = tokens[0];
    let args = tokens.into_iter().skip(1).collect();

    Ok(TokenizedCommand {
        command,
        args,
        raw: input,
    })
}

/// Tokenize a string into individual tokens.
///
/// Lower-level function that returns all tokens including the command.
pub fn tokenize_string(input: &str) -> Result<Vec<&str>, TokenizeError> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some((start, c)) = chars.next() {
        match c {
            // Skip whitespace
            ' ' | '\t' | '\r' | '\n' => continue,

            // Quoted string (double quotes)
            '"' => {
                let content_start = start + 1;
                let mut end = content_start;
                let mut found_end = false;

                while let Some((i, ch)) = chars.next() {
                    match ch {
                        '"' => {
                            found_end = true;
                            break;
                        }
                        '\\' => {
                            // Skip escaped character
                            if chars.next().is_some() {
                                end = chars.peek().map(|(i, _)| *i).unwrap_or(input.len());
                            }
                        }
                        _ => {
                            end = i + ch.len_utf8();
                        }
                    }
                }

                if !found_end {
                    return Err(TokenizeError::UnterminatedString { position: start });
                }

                tokens.push(&input[content_start..end]);
            }

            // Quoted string (single quotes)
            '\'' => {
                let content_start = start + 1;
                let mut end = content_start;
                let mut found_end = false;

                while let Some((i, ch)) = chars.next() {
                    match ch {
                        '\'' => {
                            found_end = true;
                            break;
                        }
                        '\\' => {
                            // Skip escaped character
                            if chars.next().is_some() {
                                end = chars.peek().map(|(i, _)| *i).unwrap_or(input.len());
                            }
                        }
                        _ => {
                            end = i + ch.len_utf8();
                        }
                    }
                }

                if !found_end {
                    return Err(TokenizeError::UnterminatedString { position: start });
                }

                tokens.push(&input[content_start..end]);
            }

            // Regular token
            _ => {
                let mut end = start + c.len_utf8();

                while let Some(&(i, ch)) = chars.peek() {
                    match ch {
                        ' ' | '\t' | '\r' | '\n' | '"' | '\'' => break,
                        _ => {
                            end = i + ch.len_utf8();
                            chars.next();
                        }
                    }
                }

                tokens.push(&input[start..end]);
            }
        }
    }

    Ok(tokens)
}

/// Split a command string by semicolons into multiple commands.
///
/// Respects quoted strings (semicolons inside quotes are preserved).
///
/// # Examples
///
/// ```
/// use bevy_console::core::split_commands;
///
/// let commands = split_commands("sv_cheats 1; noclip; god");
/// assert_eq!(commands, vec!["sv_cheats 1", "noclip", "god"]);
///
/// // Semicolons in quotes are preserved
/// let commands = split_commands(r#"echo "hello; world"; quit"#);
/// assert_eq!(commands, vec![r#"echo "hello; world""#, "quit"]);
/// ```
pub fn split_commands(input: &str) -> Vec<&str> {
    let mut commands = Vec::new();
    let mut start = 0;
    let mut in_double_quote = false;
    let mut in_single_quote = false;
    let mut backslash_count = 0;

    for (i, c) in input.char_indices() {
        match c {
            '\\' => {
                backslash_count += 1;
                continue;
            }
            '"' if !in_single_quote => {
                // Quote is escaped only if preceded by odd number of backslashes
                if backslash_count % 2 == 0 {
                    in_double_quote = !in_double_quote;
                }
            }
            '\'' if !in_double_quote => {
                if backslash_count % 2 == 0 {
                    in_single_quote = !in_single_quote;
                }
            }
            ';' if !in_double_quote && !in_single_quote => {
                let cmd = input[start..i].trim();
                if !cmd.is_empty() {
                    commands.push(cmd);
                }
                start = i + 1;
            }
            _ => {}
        }
        backslash_count = 0;
    }

    // Add the last command
    let cmd = input[start..].trim();
    if !cmd.is_empty() {
        commands.push(cmd);
    }

    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_simple() {
        let result = tokenize("echo hello world").unwrap();
        assert_eq!(result.command, "echo");
        assert_eq!(result.args, vec!["hello", "world"]);
    }

    #[test]
    fn test_tokenize_no_args() {
        let result = tokenize("quit").unwrap();
        assert_eq!(result.command, "quit");
        assert!(result.args.is_empty());
    }

    #[test]
    fn test_tokenize_double_quotes() {
        let result = tokenize(r#"say "hello world""#).unwrap();
        assert_eq!(result.command, "say");
        assert_eq!(result.args, vec!["hello world"]);
    }

    #[test]
    fn test_tokenize_single_quotes() {
        let result = tokenize("say 'hello world'").unwrap();
        assert_eq!(result.command, "say");
        assert_eq!(result.args, vec!["hello world"]);
    }

    #[test]
    fn test_tokenize_mixed_quotes() {
        let result = tokenize(r#"bind F1 "toggle sv_cheats""#).unwrap();
        assert_eq!(result.command, "bind");
        assert_eq!(result.args, vec!["F1", "toggle sv_cheats"]);
    }

    #[test]
    fn test_tokenize_empty() {
        assert!(matches!(tokenize(""), Err(TokenizeError::EmptyInput)));
        assert!(matches!(tokenize("   "), Err(TokenizeError::EmptyInput)));
    }

    #[test]
    fn test_tokenize_unterminated_string() {
        let result = tokenize(r#"echo "hello"#);
        assert!(matches!(result, Err(TokenizeError::UnterminatedString { .. })));
    }

    #[test]
    fn test_tokenize_comment() {
        let result = tokenize("echo hello // this is a comment").unwrap();
        assert_eq!(result.command, "echo");
        assert_eq!(result.args, vec!["hello"]);
    }

    #[test]
    fn test_tokenize_extra_whitespace() {
        let result = tokenize("  echo   hello    world  ").unwrap();
        assert_eq!(result.command, "echo");
        assert_eq!(result.args, vec!["hello", "world"]);
    }

    #[test]
    fn test_tokenize_numbers() {
        let result = tokenize("sv_gravity 800.5").unwrap();
        assert_eq!(result.command, "sv_gravity");
        assert_eq!(result.args, vec!["800.5"]);
    }

    #[test]
    fn test_tokenize_escaped_quote_in_string() {
        // Escaped quote should not close the string
        // Note: tokenize returns raw slices, so backslash is preserved
        let result = tokenize(r#"echo "hello\"world""#).unwrap();
        assert_eq!(result.command, "echo");
        assert_eq!(result.args, vec![r#"hello\"world"#]);
    }

    #[test]
    fn test_tokenize_escaped_backslash_in_string() {
        let result = tokenize(r#"echo "path\\to\\file""#).unwrap();
        assert_eq!(result.command, "echo");
        assert_eq!(result.args, vec![r#"path\\to\\file"#]);
    }

    #[test]
    fn test_tokenize_string_empty() {
        let result = tokenize_string("").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_tokenize_string_whitespace_only() {
        let result = tokenize_string("   \t\n  ").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_split_commands_simple() {
        let commands = split_commands("sv_cheats 1; noclip; god");
        assert_eq!(commands, vec!["sv_cheats 1", "noclip", "god"]);
    }

    #[test]
    fn test_split_commands_quoted() {
        let commands = split_commands(r#"echo "hello; world"; quit"#);
        assert_eq!(commands, vec![r#"echo "hello; world""#, "quit"]);
    }

    #[test]
    fn test_split_commands_single() {
        let commands = split_commands("quit");
        assert_eq!(commands, vec!["quit"]);
    }

    #[test]
    fn test_split_commands_empty() {
        let commands = split_commands("");
        assert!(commands.is_empty());
    }

    #[test]
    fn test_split_commands_semicolon_only() {
        let commands = split_commands(";;;");
        assert!(commands.is_empty());
    }

    #[test]
    fn test_split_commands_escaped_backslash_before_quote() {
        // \\" means escaped backslash followed by unescaped quote (closes string)
        let commands = split_commands(r#"echo "test\\"; quit"#);
        assert_eq!(commands, vec![r#"echo "test\\""#, "quit"]);
    }

    #[test]
    fn test_split_commands_escaped_quote() {
        // \" means escaped quote (doesn't close string)
        let commands = split_commands(r#"echo "test\"inside"; quit"#);
        assert_eq!(commands, vec![r#"echo "test\"inside""#, "quit"]);
    }

    #[test]
    fn test_split_commands_multiple_backslashes() {
        // \\\\" = 4 backslashes + quote = 2 escaped backslashes, unescaped quote
        let commands = split_commands(r#"echo "test\\\\"; quit"#);
        assert_eq!(commands, vec![r#"echo "test\\\\""#, "quit"]);

        // \\\" = 3 backslashes + quote = 1 escaped backslash, escaped quote
        let commands = split_commands(r#"echo "test\\\"inside"; quit"#);
        assert_eq!(commands, vec![r#"echo "test\\\"inside""#, "quit"]);
    }
}
