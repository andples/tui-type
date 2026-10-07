//! Custom word sets: lists of words players make themselves, type instead
//! of a language, and can publish for others to install. The rules for a
//! valid set live here so the client and the server agree on them.

/// Most words in one set.
pub const MAX_WORDS: usize = 5000;
/// Longest word, in characters.
pub const MAX_WORD_CHARS: usize = 40;
/// Name length limits.
pub const NAME_CHARS: std::ops::RangeInclusive<usize> = 2..=32;

/// A set's name: lowercase letters, digits, `_` and `-`, starting with a
/// letter or digit. Names are also file names, so nothing else goes.
pub fn valid_name(name: &str) -> bool {
    NAME_CHARS.contains(&name.chars().count())
        && name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Why `word` can't go in a set, if it can't.
pub fn word_problem(word: &str) -> Option<&'static str> {
    if word.is_empty() {
        Some("empty")
    } else if word.chars().count() > MAX_WORD_CHARS {
        Some("too long")
    } else if word.chars().any(|c| c.is_whitespace() || c.is_control()) {
        Some("has spaces or control characters")
    } else {
        None
    }
}

/// What typing `input` into a set that already has `existing` would do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Addition {
    /// New words, in the order typed, each once.
    pub new: Vec<String>,
    /// Typed words the set already has (or typed twice).
    pub duplicates: Vec<String>,
    /// Words that can't go in a set, with why.
    pub rejected: Vec<(String, &'static str)>,
    /// New words that didn't fit under `MAX_WORDS`.
    pub over_limit: usize,
}

/// Split `input` on whitespace and sort the words into new, duplicate and
/// rejected against `existing`.
pub fn addition(existing: &[String], input: &str) -> Addition {
    let mut a = Addition::default();
    let mut room = MAX_WORDS.saturating_sub(existing.len());
    for word in input.split_whitespace() {
        if let Some(why) = word_problem(word) {
            a.rejected.push((word.to_string(), why));
        } else if existing.iter().any(|w| w == word) || a.new.iter().any(|w| w == word) {
            a.duplicates.push(word.to_string());
        } else if room == 0 {
            a.over_limit += 1;
        } else {
            room -= 1;
            a.new.push(word.to_string());
        }
    }
    a
}

/// Check a whole set before it's saved or published.
pub fn check(name: &str, words: &[String]) -> Result<(), String> {
    if !valid_name(name) {
        return Err(format!(
            "`{name}` isn't a valid name: 2-32 lowercase letters, digits, _ or -"
        ));
    }
    if words.is_empty() {
        return Err("a set needs at least one word".into());
    }
    if words.len() > MAX_WORDS {
        return Err(format!("a set holds at most {MAX_WORDS} words"));
    }
    if let Some((w, why)) = words
        .iter()
        .find_map(|w| word_problem(w).map(|why| (w, why)))
    {
        return Err(format!("`{w}` can't go in a set: {why}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_plain_and_short() {
        assert!(valid_name("my-words_2"));
        assert!(valid_name("go"));
        assert!(!valid_name("a"));
        assert!(!valid_name("Caps"));
        assert!(!valid_name("-dash"));
        assert!(!valid_name("../x"));
        assert!(!valid_name(&"x".repeat(33)));
    }

    #[test]
    fn typed_words_are_sorted_into_new_duplicate_and_rejected() {
        let existing = vec!["cat".to_string()];
        let long = "y".repeat(MAX_WORD_CHARS + 1);
        let a = addition(&existing, &format!("  dog cat bird dog {long}\tfish "));
        assert_eq!(a.new, ["dog", "bird", "fish"]);
        assert_eq!(a.duplicates, ["cat", "dog"]);
        assert_eq!(a.rejected, [(long, "too long")]);
        assert_eq!(a.over_limit, 0);
        assert_eq!(addition(&existing, "   "), Addition::default());

        let full: Vec<String> = (0..MAX_WORDS - 1).map(|i| format!("w{i}")).collect();
        let a = addition(&full, "one two three");
        assert_eq!((a.new.len(), a.over_limit), (1, 2));
    }

    #[test]
    fn whole_sets_are_checked() {
        let words = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(check("ok", &words(&["a", "b"])).is_ok());
        assert!(check("Bad", &words(&["a"])).is_err());
        assert!(check("ok", &[]).is_err());
        assert!(check("ok", &words(&["a b"])).is_err());
    }
}
