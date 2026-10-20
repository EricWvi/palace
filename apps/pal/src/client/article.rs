//! Article imports use one envelope so Palace owns media staging and transaction boundaries.
use super::{Client, expect};
use crate::{
    article::Upload,
    failure::{Failure, Operation},
};
use reqwest::{StatusCode, blocking::multipart, header::AUTHORIZATION};
use serde::de::IgnoredAny;

impl Client {
    /// Sends the already validated directory once and returns Palace's actual result unchanged.
    pub(crate) fn upload_article(&self, article: Upload) -> Result<String, Failure> {
        let json = serde_json::to_string(&article.fields).map_err(Failure::UnreadableArticle)?;
        let part = multipart::Part::text(json)
            .mime_str("application/json")
            .map_err(Failure::Request)?;
        let mut form = multipart::Form::new().part("article", part);
        for media in article.files {
            let content_type = mime_guess::from_path(&media.name).first_or_octet_stream();
            let part = multipart::Part::reader_with_length(media.file, media.length)
                .mime_str(content_type.essence_str())
                .map_err(Failure::Request)?
                .file_name(media.name);
            form = form.part("file", part);
        }
        let response = self
            .http
            .post(self.config.endpoint(&["api", "articles"]))
            .header(AUTHORIZATION, self.config.authorization().clone())
            .multipart(form)
            .send()
            .map_err(|source| Failure::Transport {
                operation: Operation::ArticleUpload,
                source,
            })?;
        let success = if response.status() == StatusCode::OK {
            StatusCode::OK
        } else {
            StatusCode::CREATED
        };
        let body = expect(response, success, Operation::ArticleUpload)?;
        serde_json::from_str::<IgnoredAny>(&body).map_err(Failure::UnreadableArticle)?;
        Ok(body.trim_end().to_owned())
    }
}
