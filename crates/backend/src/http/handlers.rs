use super::{
    ApiError, LoginProvider, Server,
    security::{LOGIN_COOKIE, SESSION_COOKIE, check_origin, cookie, set_cookie},
};
use axum::{
    Json,
    extract::{Multipart, Path, Query, State, rejection::JsonRejection},
    http::{HeaderMap, header},
    response::{IntoResponse, Redirect, Response},
};
use palace_db::{AuthenticatedSession, RevokeScope};
use palace_domain::{ImportInput, ImportRequest, InputError, InputErrorKind, Source};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

/// Reads the persisted session before a handler can establish its owner scope.
pub(super) async fn authenticate<P: LoginProvider>(
    server: &Server<P>,
    headers: &HeaderMap,
) -> Result<AuthenticatedSession, ApiError> {
    let secret = cookie(headers, SESSION_COOKIE)?.ok_or(ApiError::Unauthorized)?;
    Ok(server
        .database
        .authenticate(
            &secret,
            &server.credential_key,
            &server.provider,
            (server.now)(),
        )
        .await?)
}
/// Adds rolling persistence and prevents intermediaries from caching authenticated data.
pub(super) fn authenticated_response(
    session: &AuthenticatedSession,
    body: impl IntoResponse,
) -> Result<Response, ApiError> {
    let mut response = body.into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        set_cookie(SESSION_COOKIE, &session.secret, 180 * 24 * 60 * 60)?,
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        "no-store".parse().map_err(|_| ApiError::Internal)?,
    );
    Ok(response)
}
/// Starts a persisted, browser-bound Authorization Code Flow.
pub(super) async fn login<P: LoginProvider>(
    State(server): State<Arc<Server<P>>>,
) -> Result<Response, ApiError> {
    let login = server.provider.authorize()?;
    let browser = openidconnect::CsrfToken::new_random().secret().clone();
    server
        .database
        .begin_login(
            &login.state,
            &browser,
            &login.proof,
            &server.credential_key,
            (server.now)(),
        )
        .await?;
    let mut response = Redirect::to(login.url.as_str()).into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        set_cookie(LOGIN_COOKIE, &browser, /*max_age*/ 600)?,
    );
    Ok(response)
}
#[derive(Deserialize)]
pub(super) struct Callback {
    code: String,
    state: String,
}
/// Consumes state before exchanging the code; successful login replaces any previous browser session.
pub(super) async fn callback<P: LoginProvider>(
    State(server): State<Arc<Server<P>>>,
    headers: HeaderMap,
    Query(query): Query<Callback>,
) -> Result<Response, ApiError> {
    let browser = cookie(&headers, LOGIN_COOKIE)?.ok_or(ApiError::Unauthorized)?;
    let proof = server
        .database
        .consume_login(
            &query.state,
            &browser,
            &server.credential_key,
            (server.now)(),
        )
        .await?;
    let tokens = server.provider.callback(query.code, &proof).await?;
    let previous = cookie(&headers, SESSION_COOKIE)?;
    let session = server
        .database
        .create_session(
            tokens,
            &server.credential_key,
            (server.now)(),
            previous.as_deref(),
        )
        .await?;
    let mut response = authenticated_response(&session, Redirect::to("/"))?;
    response.headers_mut().append(
        header::SET_COOKIE,
        set_cookie(LOGIN_COOKIE, "", /*max_age*/ 0)?,
    );
    Ok(response)
}
/// Returns the server-resolved current owner, renewing only an authenticated opaque cookie.
pub(super) async fn me<P: LoginProvider>(
    State(server): State<Arc<Server<P>>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let session = authenticate(&server, &headers).await?;
    authenticated_response(&session, Json(&session.owner))
}
/// Persists current-device logout before attempting external provider revocation.
pub(super) async fn logout<P: LoginProvider>(
    State(server): State<Arc<Server<P>>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    revoke(&server, headers, RevokeScope::Current).await
}
/// Uses the same authenticated boundary to revoke all devices owned by this user.
pub(super) async fn logout_all<P: LoginProvider>(
    State(server): State<Arc<Server<P>>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    revoke(&server, headers, RevokeScope::AllDevices).await
}
/// Clears the browser credential even if the external revocation endpoint is unavailable.
async fn revoke<P: LoginProvider>(
    server: &Server<P>,
    headers: HeaderMap,
    scope: RevokeScope,
) -> Result<Response, ApiError> {
    check_origin(&headers, &server.origin)?;
    let secret = cookie(&headers, SESSION_COOKIE)?.ok_or(ApiError::Unauthorized)?;
    server
        .database
        .logout_browser(&secret, scope, (server.now)())
        .await?;
    let _ = server
        .database
        .retry_revocations(&server.credential_key, &server.provider)
        .await;
    let mut response = Json(serde_json::json!({"logged_out":true})).into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        set_cookie(SESSION_COOKIE, "", /*max_age*/ 0)?,
    );
    Ok(response)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TextImport {
    title: String,
    source: Source,
    session_id: String,
    history: String,
    idempotency_key: String,
}
/// Adapts pasted JSON to the shared byte parser; owner fields are not part of this protocol.
pub(super) async fn import_text<P: LoginProvider>(
    State(server): State<Arc<Server<P>>>,
    headers: HeaderMap,
    body: Result<Json<TextImport>, JsonRejection>,
) -> Result<Response, ApiError> {
    check_origin(&headers, &server.origin)?;
    let session = authenticate(&server, &headers).await?;
    let Json(body) = body.map_err(|e| {
        InputError::new(
            if e.status() == axum::http::StatusCode::PAYLOAD_TOO_LARGE {
                InputErrorKind::Limit
            } else {
                InputErrorKind::Field
            },
            "request",
            e.body_text(),
        )
    })?;
    let request = ImportRequest::parse(
        ImportInput {
            title: body.title,
            source: body.source,
            session_id: body.session_id,
            history: body.history.into_bytes(),
            idempotency_key: body.idempotency_key,
        },
        server.limits,
    )?;
    let result = server
        .database
        .import_path(session.owner.scope(), &request)
        .await?;
    authenticated_response(&session, Json(result))
}
/// Accumulates a bounded file body and sends precisely the same bytes through the common parser.
pub(super) async fn import_file<P: LoginProvider>(
    State(server): State<Arc<Server<P>>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Response, ApiError> {
    check_origin(&headers, &server.origin)?;
    let session = authenticate(&server, &headers).await?;
    let mut fields = std::collections::HashMap::new();
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| InputError::new(InputErrorKind::Field, "multipart", e.to_string()))?
    {
        let name = field
            .name()
            .ok_or_else(|| InputError::new(InputErrorKind::Field, "multipart", "unnamed field"))?
            .to_owned();
        if ![
            "title",
            "source",
            "session_id",
            "history",
            "idempotency_key",
        ]
        .contains(&name.as_str())
            || fields.contains_key(&name)
        {
            return Err(
                InputError::new(InputErrorKind::Field, name, "unknown or repeated field").into(),
            );
        }
        let max = if name == "history" {
            server.limits.bytes
        } else {
            1024
        };
        let mut bytes = Vec::new();
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|e| InputError::new(InputErrorKind::Limit, &name, e.to_string()))?
        {
            if bytes.len().saturating_add(chunk.len()) > max {
                return Err(
                    InputError::new(InputErrorKind::Limit, &name, "field too large").into(),
                );
            }
            bytes.extend_from_slice(&chunk);
        }
        fields.insert(name, bytes);
    }
    let history = fields
        .remove("history")
        .ok_or_else(|| InputError::new(InputErrorKind::Field, "history", "missing field"))?;
    let mut text = |name: &str| -> Result<String, ApiError> {
        String::from_utf8(
            fields
                .remove(name)
                .ok_or_else(|| InputError::new(InputErrorKind::Field, name, "missing field"))?,
        )
        .map_err(|_| InputError::new(InputErrorKind::Field, name, "expected UTF-8").into())
    };
    let source = serde_json::from_value(serde_json::Value::String(text("source")?))
        .map_err(|_| InputError::new(InputErrorKind::Field, "source", "unsupported source"))?;
    let request = ImportRequest::parse(
        ImportInput {
            title: text("title")?,
            source,
            session_id: text("session_id")?,
            history,
            idempotency_key: text("idempotency_key")?,
        },
        server.limits,
    )?;
    let result = server
        .database
        .import_path(session.owner.scope(), &request)
        .await?;
    authenticated_response(&session, Json(result))
}
/// Returns a stable tree and controlled external link only for the authenticated owner.
pub(super) async fn conversation<P: LoginProvider>(
    State(server): State<Arc<Server<P>>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let session = authenticate(&server, &headers).await?;
    let (conversation, messages) = server
        .database
        .conversation(session.owner.scope(), id)
        .await?;
    let link = server
        .links
        .original_link(conversation.source, &conversation.session_id)?;
    authenticated_response(
        &session,
        Json(
            serde_json::json!({"conversation":conversation,"messages":messages,"original_link":link.as_str()}),
        ),
    )
}
/// Exposes one validated ancestor path without inferring turns or role alternation.
pub(super) async fn path<P: LoginProvider>(
    State(server): State<Arc<Server<P>>>,
    headers: HeaderMap,
    Path((id, head)): Path<(Uuid, Uuid)>,
) -> Result<Response, ApiError> {
    let session = authenticate(&server, &headers).await?;
    let path = server
        .database
        .path(session.owner.scope(), id, head)
        .await?;
    authenticated_response(&session, Json(path))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Rename {
    title: String,
}
/// Updates a title without exposing source, parent or owner reassignment through the request body.
pub(super) async fn rename<P: LoginProvider>(
    State(server): State<Arc<Server<P>>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(input): Json<Rename>,
) -> Result<Response, ApiError> {
    check_origin(&headers, &server.origin)?;
    let session = authenticate(&server, &headers).await?;
    server
        .database
        .rename_conversation(session.owner.scope(), id, &input.title)
        .await?;
    authenticated_response(
        &session,
        Json(serde_json::json!({"id":id,"title":input.title})),
    )
}
