//! Media embedded in 摘星 articles. An import stores its files before the article exists, and
//! object storage cannot join a database transaction, so the order is fixed: write every object,
//! then register the article and all its media rows in one transaction, and remove the objects
//! again whenever the import ends any other way. Deleting reverses it: objects first, then rows.
use crate::{MediaError, MediaStore, object_key};
use bytes::Bytes;
use palace_db::{ArticleImport, Database, OwnerScope, StoredMedia};
use palace_domain::{
    ArticleBody, ArticleMetadata, InputError, InputErrorKind, MediaMetadata, MediaName, MediaUpload,
};
use palace_logging::palace_warn;
use std::collections::HashMap;
use uuid::Uuid;

/// An import in progress: the objects it has written so far. Every path out of an import must
/// end in [`ArticleStaging::commit`] or [`ArticleStaging::discard`], or its objects stay behind
/// with nothing referencing them.
pub struct ArticleStaging<'a, S> {
    database: &'a Database,
    store: Option<&'a S>,
    owner: OwnerScope,
    staged: Vec<Staged>,
}

/// One written object waiting for its row.
struct Staged {
    name: MediaName,
    id: Uuid,
    key: String,
    metadata: MediaMetadata,
}

impl<'a, S: MediaStore> ArticleStaging<'a, S> {
    /// Starts an import for `owner`. Without storage an import can still save an article that
    /// embeds no files.
    pub fn new(database: &'a Database, store: Option<&'a S>, owner: OwnerScope) -> Self {
        Self {
            database,
            store,
            owner,
            staged: Vec::new(),
        }
    }

    /// Writes one file's object under a fresh media id. A name the import already used is
    /// refused before anything is written.
    pub async fn stage(&mut self, name: MediaName, upload: MediaUpload) -> Result<(), MediaError> {
        let store = self.store.ok_or(MediaError::Unconfigured)?;
        if self.staged.iter().any(|staged| staged.name == name) {
            return Err(InputError::new(
                InputErrorKind::Field,
                "file",
                format!("{} is sent twice", name.as_str()),
            )
            .into());
        }
        let id = Uuid::now_v7();
        let key = object_key(self.owner.id(), id);
        let (content, metadata) = upload.into_parts();
        store
            .put(&key, Bytes::from(content), metadata.content_type())
            .await?;
        self.staged.push(Staged {
            name,
            id,
            key,
            metadata,
        });
        Ok(())
    }

    /// Points the body at the stored files and saves the article with them. When the owner
    /// already has the URL, that article is returned and this import's objects are removed.
    pub async fn commit(
        self,
        metadata: &ArticleMetadata,
        body: ArticleBody,
    ) -> Result<ArticleImport, MediaError> {
        let files: HashMap<MediaName, Uuid> = self
            .staged
            .iter()
            .map(|staged| (staged.name.clone(), staged.id))
            .collect();
        let content = match body.resolve(&files) {
            Ok(content) => content,
            Err(error) => {
                self.discard().await;
                return Err(error.into());
            }
        };
        let stored: Vec<StoredMedia> = self
            .staged
            .iter()
            .map(|staged| StoredMedia {
                id: staged.id,
                object_key: &staged.key,
                metadata: &staged.metadata,
            })
            .collect();
        let inserted = self
            .database
            .insert_article(self.owner, metadata, &content, &stored)
            .await;
        match inserted {
            Ok(ArticleImport::Created(article)) => Ok(ArticleImport::Created(article)),
            Ok(ArticleImport::Existing(article)) => {
                self.discard().await;
                Ok(ArticleImport::Existing(article))
            }
            Err(error) => {
                self.discard().await;
                Err(error.into())
            }
        }
    }

    /// Removes every object this import wrote. A removal that fails leaves an object no row
    /// refers to, for a later reconciliation by owner prefix; the import has failed either way.
    pub async fn discard(self) {
        let Some(store) = self.store else {
            return;
        };
        for staged in self.staged {
            if let Err(error) = store.delete(&staged.key).await {
                palace_warn!(message = "Orphaned article media object after a failed import", key = %staged.key, error = %error);
            }
        }
    }
}

/// Deletes an article with its media: every object first, then the rows in one transaction. A
/// failure leaves the article in place, perhaps missing some images, and a retry finishes it.
pub async fn delete_article<S: MediaStore>(
    database: &Database,
    store: Option<&S>,
    owner: OwnerScope,
    id: Uuid,
) -> Result<(), MediaError> {
    let keys = database.article_object_keys(owner, id).await?;
    if !keys.is_empty() {
        let store = store.ok_or(MediaError::Unconfigured)?;
        for key in &keys {
            if let Err(error) = store.delete(key).await {
                palace_warn!(message = "Article media delete failed; the article is kept for a retry", key = %key, error = %error);
                return Err(error.into());
            }
        }
    }
    database.delete_article(owner, id).await?;
    Ok(())
}
