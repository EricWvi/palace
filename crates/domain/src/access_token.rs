//! Owner access tokens: the note a user gives a token and the token's literal format.
use crate::{InputError, InputErrorKind};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};

/// Lets people and secret scanners recognize a leaked Palace credential at a glance.
pub const ACCESS_TOKEN_PREFIX: &str = "palace_pat_";
// 32 random bytes encode to 43 unpadded base64url characters.
const RANDOM_CHARS: usize = 43;
// Enough of each end for a user to match a list row to the token in their configuration.
const VISIBLE_CHARS: usize = 4;
const NOTE_CHARS: usize = 100;

/// A trimmed, nonblank label telling the owner what a token is used for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessTokenNote(String);
impl AccessTokenNote {
    /// Accepts at most 100 characters after trimming surrounding whitespace.
    pub fn parse(raw: &str) -> Result<Self, InputError> {
        let note = raw.trim();
        if note.is_empty() {
            return Err(InputError::new(
                InputErrorKind::Field,
                "note",
                "note is required",
            ));
        }
        if note.chars().count() > NOTE_CHARS {
            return Err(InputError::new(
                InputErrorKind::Limit,
                "note",
                format!("note exceeds {NOTE_CHARS} characters"),
            ));
        }
        Ok(Self(note.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A complete access token. It is only ever held while being issued or authenticated, so it
/// deliberately implements neither `Debug` nor `Serialize`.
pub struct AccessTokenSecret(String);
impl AccessTokenSecret {
    /// Encodes 256 bits of caller-supplied randomness; the caller owns the CSPRNG.
    pub fn from_random(bytes: [u8; 32]) -> Self {
        Self(format!(
            "{ACCESS_TOKEN_PREFIX}{}",
            URL_SAFE_NO_PAD.encode(bytes)
        ))
    }
    /// Recognizes only the exact issued shape, so malformed credentials fail before any lookup.
    pub fn parse(value: &str) -> Option<Self> {
        let random = value.strip_prefix(ACCESS_TOKEN_PREFIX)?;
        (random.len() == RANDOM_CHARS
            && random
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
        .then(|| Self(value.to_owned()))
    }
    /// The full value, to be returned exactly once in the creation response.
    pub fn expose(&self) -> &str {
        &self.0
    }
    /// A single SHA-256 suffices for a 256-bit random value; a slow hash would only add latency.
    pub fn digest(&self) -> Vec<u8> {
        Sha256::digest(self.0.as_bytes()).to_vec()
    }
    /// Keeps the prefix and both ends of the random part; every other character becomes `*`.
    pub fn masked(&self) -> String {
        let random = &self.0[ACCESS_TOKEN_PREFIX.len()..];
        let hidden = random.len() - 2 * VISIBLE_CHARS;
        format!(
            "{ACCESS_TOKEN_PREFIX}{}{}{}",
            &random[..VISIBLE_CHARS],
            "*".repeat(hidden),
            &random[random.len() - VISIBLE_CHARS..]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Fixes the literal format, the masked form and which values authenticate at all.
    /// Core test case:
    /// - `specs/test-cases/server/owner/access-tokens.md#the-full-access-token-must-leave-the-server-only-in-its-creation-response`
    #[test]
    fn tokens_have_one_shape_and_mask_all_but_their_ends() {
        let token = AccessTokenSecret::from_random([0xfb; 32]);
        let random = "-_v7-_v7-_v7-_v7-_v7-_v7-_v7-_v7-_v7-_v7-_s";
        assert_eq!(token.expose(), format!("palace_pat_{random}"));
        assert_eq!(
            token.masked(),
            format!("palace_pat_-_v7{}7-_s", "*".repeat(35))
        );
        assert_eq!(
            AccessTokenSecret::parse(token.expose()).map(|parsed| parsed.digest()),
            Some(token.digest())
        );
        for invalid in [
            String::new(),
            random.to_owned(),
            format!("palace_pat_{}", &random[1..]),
            format!("palace_pat_{random}A"),
            format!("palace_pat_{}=", &random[1..]),
            format!("palace_pat_{}+", &random[1..]),
            format!("Palace_pat_{random}"),
        ] {
            assert!(AccessTokenSecret::parse(&invalid).is_none(), "{invalid}");
        }
    }

    /// Notes are trimmed, required and bounded by characters rather than bytes.
    #[test]
    fn notes_are_trimmed_nonblank_and_bounded() {
        assert_eq!(
            AccessTokenNote::parse("  cli  ").map(|note| note.as_str().to_owned()),
            Ok("cli".to_owned())
        );
        assert_eq!(
            AccessTokenNote::parse(&"密".repeat(100)).map(|note| note.as_str().chars().count()),
            Ok(100)
        );
        assert_eq!(
            [
                AccessTokenNote::parse(" \n").map_err(|e| e.kind),
                AccessTokenNote::parse(&"密".repeat(101)).map_err(|e| e.kind),
            ],
            [Err(InputErrorKind::Field), Err(InputErrorKind::Limit)]
        );
    }
}
