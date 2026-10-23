//! Order keys in the format of rocicorp's `fractional-indexing`, which the page uses to write
//! them. The server never makes a key; it only refuses one the library would not accept, so a
//! stored key always works as a neighbour for the next `generateKeyBetween`.
use crate::{InputError, InputErrorKind};

const POSITION_BYTES: usize = 1024;
const DIGITS: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// A task's place among its siblings: keys compare as bytes, smallest on top.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoPosition(String);

impl TodoPosition {
    /// Accepts a key the library's `validateOrderKey` accepts, of at most 1 KiB.
    pub fn parse(key: String) -> Result<Self, InputError> {
        if !is_order_key(&key) || key.len() > POSITION_BYTES {
            return Err(InputError::new(
                InputErrorKind::Field,
                "position",
                "expected a fractional-indexing key of at most 1024 bytes",
            ));
        }
        Ok(Self(key))
    }

    /// Rebuilds a key the database already holds; it was checked when written.
    pub fn from_stored(key: String) -> Self {
        Self(key)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An integer part whose head letter encodes its length (`a`..`z` count up from 2 characters,
/// `Z`..`A` down from 2), then a base-62 fraction with no trailing zero. The smallest integer,
/// `A` and 26 zeros, is reserved by the library so a key below every other always exists.
fn is_order_key(key: &str) -> bool {
    let Some(head) = key.chars().next() else {
        return false;
    };
    let integer = match head {
        'a'..='z' => head as usize - 'a' as usize + 2,
        'A'..='Z' => 'Z' as usize - head as usize + 2,
        _ => return false,
    };
    if key.len() < integer || !key[1..].chars().all(|c| DIGITS.contains(c)) {
        return false;
    }
    let smallest = key.len() == 27 && key.starts_with('A') && key[1..].bytes().all(|b| b == b'0');
    !smallest && !key[integer..].ends_with('0')
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Keys the library makes pass; malformed ones are refused by field.
    /// Core test case:
    /// - `specs/test-cases/web/todo/todos.md#task-order-must-come-from-unique-well-formed-keys`
    #[test]
    fn keys_follow_the_library_format() {
        for key in [
            "a0",
            "a1",
            "a0V",
            "Zz",
            "b10",
            "a0i5",
            "zzzzzzzzzzzzzzzzzzzzzzzzzzz",
        ] {
            assert_eq!(
                TodoPosition::parse(key.into()),
                Ok(TodoPosition(key.into())),
                "{key}"
            );
        }
        let smallest = format!("A{}", "0".repeat(26));
        let long = format!("a0{}", "V".repeat(1023));
        for key in ["", "a", "b1", "a00", "0a", "a-1", "a0 ", &smallest, &long] {
            assert_eq!(
                TodoPosition::parse(key.into()).map_err(|error| error.path),
                Err("position".into()),
                "{key}"
            );
        }
    }
}
