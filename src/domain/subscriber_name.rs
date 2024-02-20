use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
pub struct SubscriberName(String);

impl SubscriberName {
	pub fn parse(s: String) -> Result<SubscriberName, String> {
		let is_empty_or_whitespace = s.trim().is_empty();
		let is_too_long = s.graphemes(true).count() > 2048;
		let forbidden_characters = [';', ':', '!', '?', '*', '(', ')', '&', '$', '@', '#', '<', '>', '[', ']', '{', '}', '/', '\\'];
		let contains_forbidden_characters = s.chars().any(|c| forbidden_characters.contains(&c));

		if is_empty_or_whitespace || is_too_long || contains_forbidden_characters
		{
			Err(format!("{} is not a valid subscriber name.", s))
		} else {
			Ok(Self(s))
		}
	}
}

impl AsRef<str> for SubscriberName {
	fn as_ref(&self) -> &str {
		&self.0
	}
}

#[cfg(test)]
mod tests {
	use crate::domain::SubscriberName;
	use claim::{assert_err, assert_ok};

	#[test]
	fn a_very_long_name_is_rejected() {
		let too_long_name = "a".repeat(2049);
		assert_err!(SubscriberName::parse(too_long_name));
	}

	#[test]
	fn a_name_with_forbidden_characters_is_rejected() {
		let forbidden_characters = vec![';', ':', '!', '?', '*', '(', ')', '&', '$', '@', '#', '<', '>', '[', ']', '{', '}', '/', '\\'];
		for forbidden_character in forbidden_characters {
			let name_with_forbidden_character = format!("name{}", forbidden_character);
			assert_err!(SubscriberName::parse(name_with_forbidden_character));
		}
	}

	#[test]
	fn a_name_with_allowed_characters_is_accepted() {
		let name = "name".to_string();
		assert_ok!(SubscriberName::parse(name));
	}

	#[test]
	fn empty_string_is_rejected() {
		let name = "".to_string();
		assert_err!(SubscriberName::parse(name));
	}
}
