use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
enum SplittedText {
    Unmatched(String),
    FullMatch(String),
    HasPrefix(String, String),
    HasSuffix(String, String),
    HasPrefixAndSuffix(String, String, String),
}
fn split_first_regex(text: String, regex: &Regex) -> SplittedText {
    let mat = regex.find(&text);
    match mat {
        Some(mat) => match (mat.start(), mat.end()) {
            (0, end) if end == text.len() => SplittedText::FullMatch(text),
            (0, end) => {
                let (target, suffix) = text.split_at(end);
                SplittedText::HasSuffix(target.to_string(), suffix.to_string())
            }
            (start, end) if end == text.len() => {
                let (prefix, target) = text.split_at(start);
                SplittedText::HasPrefix(prefix.to_string(), target.to_string())
            }
            (start, end) => {
                let (prefix, rest) = text.split_at(start);
                let (target, suffix) = rest.split_at(end - start);
                SplittedText::HasPrefixAndSuffix(
                    prefix.to_string(),
                    target.to_string(),
                    suffix.to_string(),
                )
            }
        },
        None => SplittedText::Unmatched(text),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SplittedElement {
    Unmatched(String),
    Matched(String),
}
pub fn split_all_regex(mut target_text: String, regex: &Regex) -> Vec<SplittedElement> {
    let mut result = Vec::new();
    loop {
        match split_first_regex(target_text, regex) {
            SplittedText::Unmatched(text) => {
                result.push(SplittedElement::Unmatched(text));
                break;
            }
            SplittedText::FullMatch(matched) => {
                result.push(SplittedElement::Matched(matched));
                break;
            }
            SplittedText::HasPrefix(prefix, matched) => {
                result.push(SplittedElement::Unmatched(prefix));
                result.push(SplittedElement::Matched(matched));
                break;
            }
            SplittedText::HasSuffix(matched, suffix) => {
                result.push(SplittedElement::Matched(matched));
                target_text = suffix;
                continue;
            }
            SplittedText::HasPrefixAndSuffix(prefix, matched, suffix) => {
                result.push(SplittedElement::Unmatched(prefix));
                result.push(SplittedElement::Matched(matched));
                target_text = suffix;
                continue;
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABC_REGEX_BASE: &str = r#"a+b+c+"#;

    #[test]
    fn test_split_first_unmatched() {
        let text = "bbbccccc".to_string();
        let result = split_first_regex(text.clone(), &Regex::new(ABC_REGEX_BASE).unwrap());
        assert_eq!(result, SplittedText::Unmatched(text));
    }

    #[test]
    fn test_split_first_full_match() {
        let text = "aaabbbbbcccc".to_string();
        let result = split_first_regex(text.clone(), &Regex::new(ABC_REGEX_BASE).unwrap());
        assert_eq!(result, SplittedText::FullMatch(text));
    }

    #[test]
    fn test_split_first_has_prefix() {
        let text = "xxxxaaabbbbbcccc".to_string();
        let result = split_first_regex(text, &Regex::new(ABC_REGEX_BASE).unwrap());
        assert_eq!(
            result,
            SplittedText::HasPrefix("xxxx".to_string(), "aaabbbbbcccc".to_string())
        );
    }

    #[test]
    fn test_split_first_has_suffix() {
        let text = "aaabbbbbccccxxxx".to_string();
        let result = split_first_regex(text, &Regex::new(ABC_REGEX_BASE).unwrap());
        assert_eq!(
            result,
            SplittedText::HasSuffix("aaabbbbbcccc".to_string(), "xxxx".to_string())
        );
    }

    #[test]
    fn test_split_first_has_prefix_and_suffix() {
        let text = "xxxxaaabbbbbccccxxxx".to_string();
        let result = split_first_regex(text, &Regex::new(ABC_REGEX_BASE).unwrap());
        assert_eq!(
            result,
            SplittedText::HasPrefixAndSuffix(
                "xxxx".to_string(),
                "aaabbbbbcccc".to_string(),
                "xxxx".to_string()
            )
        );
    }

    #[test]
    fn test_split_all_unmatched() {
        let text = "bbbccccc".to_string();
        let result = split_all_regex(text.clone(), &Regex::new(ABC_REGEX_BASE).unwrap());
        assert_eq!(result, vec![SplittedElement::Unmatched(text)]);
    }

    #[test]
    fn test_split_all_full_match() {
        let text = "aaabbbbbcccc".to_string();
        let result = split_all_regex(text.clone(), &Regex::new(ABC_REGEX_BASE).unwrap());
        assert_eq!(result, vec![SplittedElement::Matched(text)]);
    }

    #[test]
    fn test_split_all_has_prefix() {
        let text = "xxxxaaabbbbbcccc".to_string();
        let result = split_all_regex(text, &Regex::new(ABC_REGEX_BASE).unwrap());
        assert_eq!(
            result,
            vec![
                SplittedElement::Unmatched("xxxx".to_string()),
                SplittedElement::Matched("aaabbbbbcccc".to_string()),
            ]
        );
    }

    #[test]
    fn test_split_all_has_suffix() {
        let text = "aaabbbbbccccxxxx".to_string();
        let result = split_all_regex(text, &Regex::new(ABC_REGEX_BASE).unwrap());
        assert_eq!(
            result,
            vec![
                SplittedElement::Matched("aaabbbbbcccc".to_string()),
                SplittedElement::Unmatched("xxxx".to_string()),
            ]
        );
    }

    #[test]
    fn test_split_all_long() {
        let text = "xxabcxxxaabbccxxxxaxbxcxabcx".to_string();
        let result = split_all_regex(text, &Regex::new(ABC_REGEX_BASE).unwrap());
        assert_eq!(
            result,
            vec![
                SplittedElement::Unmatched("xx".to_string()),
                SplittedElement::Matched("abc".to_string()),
                SplittedElement::Unmatched("xxx".to_string()),
                SplittedElement::Matched("aabbcc".to_string()),
                SplittedElement::Unmatched("xxxxaxbxcx".to_string()),
                SplittedElement::Matched("abc".to_string()),
                SplittedElement::Unmatched("x".to_string()),
            ]
        );
    }

    #[test]
    fn test_split_all_longest_match() {
        let regex = Regex::new(r#"\{.*\}"#).unwrap();
        let text = "xx{XXX}yyy{YYY}zzz{ZZZ}www".to_string();
        let result = split_all_regex(text, &regex);
        assert_eq!(
            result,
            vec![
                SplittedElement::Unmatched("xx".to_string()),
                SplittedElement::Matched("{XXX}yyy{YYY}zzz{ZZZ}".to_string()),
                SplittedElement::Unmatched("www".to_string()),
            ]
        );
    }

    #[test]
    fn test_split_all_shortest_match() {
        let regex = Regex::new(r#"\{.*?\}"#).unwrap();
        let text = "xx{XXX}yyy{YYY}zzz{ZZZ}www".to_string();
        let result = split_all_regex(text, &regex);
        assert_eq!(
            result,
            vec![
                SplittedElement::Unmatched("xx".to_string()),
                SplittedElement::Matched("{XXX}".to_string()),
                SplittedElement::Unmatched("yyy".to_string()),
                SplittedElement::Matched("{YYY}".to_string()),
                SplittedElement::Unmatched("zzz".to_string()),
                SplittedElement::Matched("{ZZZ}".to_string()),
                SplittedElement::Unmatched("www".to_string()),
            ]
        );
    }
}
