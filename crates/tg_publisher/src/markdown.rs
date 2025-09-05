/// Escape text for Telegram MarkdownV2.
/// See <https://core.telegram.org/bots/api#markdownv2-style>
/// Characters: _ * [ ] ( ) ~ ` > # + - = | { } . ! and backslash.
pub fn escape_md_v2(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + s.len()/8);
    for ch in s.chars() {
        match ch {
            '_' | '*' | '[' | ']' | '(' | ')' | '~' | '`' | '>' | '#' |
            '+' | '-' | '=' | '|' | '{' | '}' | '.' | '!' | '\\' => {
                out.push('\\'); out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escape_basic() {
        let s = "_test.*(ok)!";
        assert_eq!(escape_md_v2(s), "\\_test\\.\\*\\(ok\\)\\!");
    }
}

