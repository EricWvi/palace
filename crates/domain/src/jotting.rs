//! 札记 (jottings): short Markdown files kept in a Git repository, identified by the UUIDv7 in
//! their frontmatter rather than by their path. The server, the `pal` checker and the upload
//! endpoint all read and write the format through this module, so a file the pre-commit check
//! accepts is exactly a file the server indexes.
//! See `specs/decisions/server/jotting/0-jottings-indexed-from-git-commits.md`.
use crate::{InputError, InputErrorKind, split_frontmatter};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use time::{OffsetDateTime, UtcOffset, format_description::well_known::Rfc3339};
use uuid::{Uuid, Version};
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader, yaml::Hash};

/// Upper bound for an uploaded body; files written by the agent are not limited.
pub const JOTTING_BODY_MAX_BYTES: usize = 1024 * 1024;

/// The fields a jotting carries to clients. Other frontmatter keys stay in the file only, so
/// editing them never publishes a change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jotting {
    pub id: Uuid,
    /// Unix epoch milliseconds of the frontmatter `created`.
    pub created_at: i64,
    pub title: Option<String>,
    pub tags: Vec<String>,
    /// Markdown after the frontmatter, without the blank lines that separate the two.
    pub body: String,
}

/// Why one file is not a jotting.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum JottingError {
    #[error("file is not UTF-8")]
    Encoding,
    #[error("file does not start with a closed `---` frontmatter block")]
    NoFrontmatter,
    #[error("frontmatter is not valid YAML: {0}")]
    Yaml(String),
    #[error("frontmatter is not a mapping")]
    NotMapping,
    #[error("`{0}` is missing")]
    Missing(&'static str),
    #[error("`id` must be a lowercase hyphenated UUIDv7")]
    Id,
    #[error("`created` must be an RFC 3339 time with an offset and at most millisecond precision")]
    Created,
    #[error("`title` must be a non-blank string when present")]
    Title,
    #[error("`tags` must be a list of distinct non-blank strings when present")]
    Tags,
}

impl Jotting {
    /// Reads one Markdown file. Fields outside the jotting model are ignored, not rejected, so
    /// the agent may keep its own bookkeeping in the frontmatter.
    pub fn parse(bytes: &[u8]) -> Result<Self, JottingError> {
        let text = std::str::from_utf8(bytes).map_err(|_| JottingError::Encoding)?;
        let (frontmatter, body) = split_frontmatter(text).ok_or(JottingError::NoFrontmatter)?;
        let documents = YamlLoader::load_from_str(frontmatter)
            .map_err(|e| JottingError::Yaml(e.to_string()))?;
        let Some(Yaml::Hash(fields)) = documents.into_iter().next() else {
            return Err(JottingError::NotMapping);
        };
        let field = |name: &str| fields.get(&Yaml::String(name.to_owned()));
        let id = match field("id") {
            None => return Err(JottingError::Missing("id")),
            Some(Yaml::String(id)) => parse_id(id).ok_or(JottingError::Id)?,
            Some(_) => return Err(JottingError::Id),
        };
        let created_at = match field("created") {
            None => return Err(JottingError::Missing("created")),
            Some(Yaml::String(created)) => parse_created(created).ok_or(JottingError::Created)?,
            Some(_) => return Err(JottingError::Created),
        };
        let title = match field("title") {
            None => None,
            Some(Yaml::String(title)) if !title.trim().is_empty() => Some(title.clone()),
            Some(_) => return Err(JottingError::Title),
        };
        let tags = match field("tags") {
            None => Vec::new(),
            Some(Yaml::Array(items)) => {
                let tags = items
                    .iter()
                    .map(|item| match item {
                        Yaml::String(tag) => Some(tag.clone()),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or(JottingError::Tags)?;
                if !tags_are_valid(&tags) {
                    return Err(JottingError::Tags);
                }
                tags
            }
            Some(_) => return Err(JottingError::Tags),
        };
        Ok(Self {
            id,
            created_at,
            title,
            tags,
            body: body.to_owned(),
        })
    }

    /// Validates an uploaded jotting and puts it in the form its rendered file parses back to,
    /// so the commit that later contains the file publishes no spurious change.
    pub fn upload(mut self) -> Result<Self, InputError> {
        let field =
            |path: &str, message: &str| InputError::new(InputErrorKind::Field, path, message);
        if self.id.get_version() != Some(Version::SortRand) {
            return Err(field("id", "expected a UUIDv7"));
        }
        if created_time(self.created_at).is_none() {
            return Err(field("created_at", "out of range"));
        }
        if self
            .title
            .as_ref()
            .is_some_and(|title| title.trim().is_empty())
        {
            return Err(field("title", "expected a non-blank title or null"));
        }
        if !tags_are_valid(&self.tags) {
            return Err(field("tags", "expected distinct non-blank tags"));
        }
        if self.body.len() > JOTTING_BODY_MAX_BYTES {
            return Err(InputError::new(
                InputErrorKind::Limit,
                "body",
                "body too large",
            ));
        }
        self.body = self.body.trim_start_matches(['\r', '\n']).to_owned();
        Ok(self)
    }

    /// Writes the jotting as a Markdown file, with `created` in the deployment's offset at that
    /// instant so the agent reads local times. Keys appear in a fixed order.
    pub fn render(&self, offset: UtcOffset) -> String {
        let mut fields = Hash::new();
        let mut put = |key: &str, value: Yaml| {
            fields.insert(Yaml::String(key.to_owned()), value);
        };
        put("id", Yaml::String(self.id.hyphenated().to_string()));
        let created = created_time(self.created_at)
            .map(|time| time.to_offset(offset))
            .and_then(|time| time.format(&Rfc3339).ok())
            .unwrap_or_default();
        put("created", Yaml::String(created));
        if let Some(title) = &self.title {
            put("title", Yaml::String(title.clone()));
        }
        if !self.tags.is_empty() {
            put(
                "tags",
                Yaml::Array(self.tags.iter().cloned().map(Yaml::String).collect()),
            );
        }
        let mut frontmatter = String::new();
        // Emitting into a String cannot fail; the emitter only reports writer errors.
        let _ = YamlEmitter::new(&mut frontmatter).dump(&Yaml::Hash(fields));
        format!("{frontmatter}\n---\n\n{}", self.body)
    }

    /// Identifies an upload by its content, independent of how it is rendered, so a retry is
    /// recognized even after the deployment's time zone changed or the file was edited.
    pub fn upload_digest(&self) -> [u8; 32] {
        let canonical = serde_json::json!([
            self.id.hyphenated().to_string(),
            self.created_at,
            self.title,
            self.tags,
            self.body,
        ]);
        Sha256::digest(canonical.to_string().as_bytes()).into()
    }
}

/// Accepts only the canonical lowercase hyphenated spelling, so one id has one spelling in files.
fn parse_id(text: &str) -> Option<Uuid> {
    let id = Uuid::try_parse(text).ok()?;
    (id.get_version() == Some(Version::SortRand) && id.hyphenated().to_string() == text)
        .then_some(id)
}

/// Requires an explicit offset and whole milliseconds, the precision every client stores.
fn parse_created(text: &str) -> Option<i64> {
    let time = OffsetDateTime::parse(text, &Rfc3339).ok()?;
    if time.nanosecond() % 1_000_000 != 0 {
        return None;
    }
    i64::try_from(time.unix_timestamp_nanos() / 1_000_000).ok()
}

/// The instant of an epoch-millisecond time, or `None` outside the range RFC 3339 can write.
fn created_time(millis: i64) -> Option<OffsetDateTime> {
    let time = OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000).ok()?;
    (0..=9999).contains(&time.year()).then_some(time)
}

/// Tags must be distinct and non-blank, so clients can treat them as a set.
fn tags_are_valid(tags: &[String]) -> bool {
    let mut seen = HashSet::new();
    tags.iter()
        .all(|tag| !tag.trim().is_empty() && seen.insert(tag.as_str()))
}

/// One reason a set of jotting files cannot be indexed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum JottingProblem {
    #[error("{path}: {error}")]
    Invalid { path: String, error: JottingError },
    #[error("{path}: id {id} is already used by {first}")]
    Duplicate {
        path: String,
        id: Uuid,
        first: String,
    },
}

/// Collects every problem of one set of files, so a refused commit reports all of them at once.
/// The server and `pal jottings check` both judge a set through this type.
#[derive(Debug, Default)]
pub struct JottingCensus {
    paths: HashMap<Uuid, String>,
    problems: Vec<JottingProblem>,
}

impl JottingCensus {
    /// Counts a file whose id is already known, without parsing it again.
    pub fn record(&mut self, path: &str, id: Uuid) {
        match self.paths.get(&id) {
            Some(first) => self.problems.push(JottingProblem::Duplicate {
                path: path.to_owned(),
                id,
                first: first.clone(),
            }),
            None => {
                self.paths.insert(id, path.to_owned());
            }
        }
    }

    /// Counts a parsed file, keeping the jotting only when it parsed.
    pub fn admit(&mut self, path: &str, parsed: Result<Jotting, JottingError>) -> Option<Jotting> {
        match parsed {
            Ok(jotting) => {
                self.record(path, jotting.id);
                Some(jotting)
            }
            Err(error) => {
                self.problems.push(JottingProblem::Invalid {
                    path: path.to_owned(),
                    error,
                });
                None
            }
        }
    }

    /// Every problem found, or success when the set can be indexed as a whole.
    pub fn finish(self) -> Result<usize, Vec<JottingProblem>> {
        if self.problems.is_empty() {
            Ok(self.paths.len())
        } else {
            Err(self.problems)
        }
    }
}

#[cfg(test)]
mod tests;
