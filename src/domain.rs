use unicode_segmentation::UnicodeSegmentation;

// Keep the String field private to make it impossible to instantiate a SubscriberName
// directly outside this module.
pub struct SubscriberName(String);

impl SubscriberName {
    pub fn parse(s: String) -> Self {
        let is_empty_or_whitespace = s.trim().is_empty();

        // A grapheme is defined by Unicode standard as a "user-perceived" character.
        // For example, `å` is a single grapheme, but it is composed of two characters: `a` and ``.
        let is_too_long = s.graphemes(true).count() > 256;

        let forbidden_characters = ['/', '(', ')', '"', '<', '>', '\\', '{', '}'];
        let contains_forbidden_characters = s.chars().any(|c| forbidden_characters.contains(&c));

        if is_empty_or_whitespace || is_too_long || contains_forbidden_characters {
            panic!("{} is not a valid subscriber name.", s)
        } else {
            Self(s)
        }
    }

    pub fn inner_ref(&self) -> &String {
        &self.0
    }
}

pub struct NewSubscriber {
    pub email: String,
    pub name: SubscriberName,
}
