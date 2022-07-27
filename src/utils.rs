use regex::Regex;

#[derive(Debug, Clone)]
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
                SplittedText::HasPrefix(target.to_string(), suffix.to_string())
            }
            (start, end) if end == text.len() => {
                let (prefix, target) = text.split_at(start);
                SplittedText::HasSuffix(prefix.to_string(), target.to_string())
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

#[derive(Debug, Clone)]
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
