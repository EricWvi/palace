use crate::{InputError, InputErrorKind};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Chatgpt,
    Gemini,
    Grok,
}

impl Source {
    /// Returns the canonical database and wire representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chatgpt => "chatgpt",
            Self::Gemini => "gemini",
            Self::Grok => "grok",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(try_from = "String", into = "String")]
pub struct SessionId(String);

impl TryFrom<String> for SessionId {
    type Error = InputError;
    /// Restricts external identities to one literal path segment before URL encoding.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty()
            || value.len() > 512
            || value == "."
            || value == ".."
            || value
                .chars()
                .any(|c| c.is_whitespace() || c.is_control() || "/\\?#%:".contains(c))
        {
            return Err(InputError::new(
                InputErrorKind::Field,
                "session_id",
                "expected one nonempty path segment",
            ));
        }
        Ok(Self(value))
    }
}
impl From<SessionId> for String {
    /// Keeps the original source identity unchanged in serialization.
    fn from(value: SessionId) -> Self {
        value.0
    }
}
impl SessionId {
    /// Borrows the source identity without normalizing it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Deployment-owned link prefixes; request data can only supply a validated final segment.
#[derive(Clone, Debug)]
pub struct SourceLinks {
    pub chatgpt: url::Url,
    pub gemini: url::Url,
    pub grok: url::Url,
}
impl SourceLinks {
    /// Builds the initial product templates without making any network requests.
    pub fn standard() -> Result<Self, url::ParseError> {
        Ok(Self {
            chatgpt: url::Url::parse("https://chatgpt.com/c/")?,
            gemini: url::Url::parse("https://gemini.google.com/app/")?,
            grok: url::Url::parse("https://grok.com/c/")?,
        })
    }
    /// Appends the identity as an encoded segment of a trusted deployment template.
    pub fn original_link(
        &self,
        source: Source,
        session: &SessionId,
    ) -> Result<url::Url, InputError> {
        let mut url = match source {
            Source::Chatgpt => self.chatgpt.clone(),
            Source::Gemini => self.gemini.clone(),
            Source::Grok => self.grok.clone(),
        };
        url.path_segments_mut()
            .map_err(|()| {
                InputError::new(
                    InputErrorKind::Field,
                    "template",
                    "expected hierarchical URL",
                )
            })?
            .pop_if_empty()
            .push(session.as_str());
        Ok(url)
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}
impl Role {
    /// Uses identical role spelling in storage and JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Conversation {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub title: String,
    pub source: Source,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Message {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub conversation_id: Uuid,
    pub parent_message_id: Option<Uuid>,
    pub role: Role,
    pub content: String,
    pub created_order: i64,
}
/// Checks the complete ancestor chain before returning any path, including corrupted stored trees.
pub fn read_path(
    conversation: &Conversation,
    messages: &[Message],
    head: Uuid,
) -> Result<Vec<Message>, InputError> {
    let index: HashMap<_, _> = messages.iter().map(|m| (m.id, m)).collect();
    let mut visited = HashSet::new();
    let mut path = Vec::new();
    let mut current = Some(head);
    while let Some(id) = current {
        let message = index
            .get(&id)
            .ok_or_else(|| InputError::new(InputErrorKind::Field, "head", "missing ancestor"))?;
        if !visited.insert(id)
            || message.owner_id != conversation.owner_id
            || message.conversation_id != conversation.id
        {
            return Err(InputError::new(
                InputErrorKind::Field,
                "head",
                "cycle or foreign ancestor",
            ));
        }
        path.push((*message).clone());
        current = message.parent_message_id;
    }
    path.reverse();
    Ok(path)
}
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    /// Proves untrusted identities cannot change URL authority or escape the template.
    #[test]
    fn controlled_links_and_template_updates_preserve_identity() {
        let mut links = SourceLinks::standard().unwrap();
        let id = SessionId::try_from("会话-1".to_owned()).unwrap();
        for (source, prefix) in [
            (Source::Chatgpt, "https://chatgpt.com/c/"),
            (Source::Gemini, "https://gemini.google.com/app/"),
            (Source::Grok, "https://grok.com/c/"),
        ] {
            assert_eq!(
                links.original_link(source, &id).unwrap().as_str(),
                format!("{prefix}%E4%BC%9A%E8%AF%9D-1")
            );
        }
        links.chatgpt = url::Url::parse("https://chatgpt.com/new/").unwrap();
        assert_eq!(
            links.original_link(Source::Chatgpt, &id).unwrap().path(),
            "/new/%E4%BC%9A%E8%AF%9D-1"
        );
        assert_eq!(id.as_str(), "会话-1");
        for input in [
            "",
            ".",
            "..",
            "x/y",
            "x\\y",
            "x?y",
            "x#y",
            "https://evil",
            "%2f",
            "a\nb",
        ] {
            assert!(SessionId::try_from(input.to_owned()).is_err());
        }
    }
    /// Rejects cycles and foreign ancestors while preserving nonalternating paths.
    #[test]
    fn path_requires_acyclic_scoped_ancestors() {
        let c = Conversation {
            id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            title: "t".into(),
            source: Source::Grok,
        };
        let a = Message {
            id: Uuid::new_v4(),
            owner_id: c.owner_id,
            conversation_id: c.id,
            parent_message_id: None,
            role: Role::User,
            content: "A".into(),
            created_order: 1,
        };
        let mut b = Message {
            id: Uuid::new_v4(),
            parent_message_id: Some(a.id),
            content: "B".into(),
            created_order: 2,
            ..a.clone()
        };
        assert_eq!(
            read_path(&c, &[a.clone(), b.clone()], b.id).unwrap(),
            vec![a.clone(), b.clone()]
        );
        b.parent_message_id = Some(b.id);
        assert!(read_path(&c, &[b.clone()], b.id).is_err());
        b.parent_message_id = Some(a.id);
        b.owner_id = Uuid::new_v4();
        assert!(read_path(&c, &[a.clone(), b.clone()], b.id).is_err());
        b.owner_id = c.owner_id;
        b.conversation_id = Uuid::new_v4();
        assert!(read_path(&c, &[a, b.clone()], b.id).is_err());
    }
}
