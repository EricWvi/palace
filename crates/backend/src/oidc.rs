use openidconnect::{
    AccessTokenHash, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet,
    EndpointNotSet, EndpointSet, IssuerUrl, Nonce, OAuth2TokenResponse, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, RefreshToken, Scope, TokenResponse,
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata, CoreTokenResponse},
};
use palace_db::{IdentityProvider, IdentityTokens, ProviderError};
use serde::{Deserialize, Serialize};

type Client = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;
#[derive(Clone)]
pub struct OidcProvider {
    client: Client,
    http: reqwest::Client,
    issuer: String,
    client_id: String,
    client_secret: String,
    revocation_url: url::Url,
}
/// Encrypted proof retained only until a single successful callback consumption.
#[derive(Serialize, Deserialize)]
pub struct LoginProof {
    nonce: String,
    verifier: String,
}
pub struct LoginRedirect {
    pub url: url::Url,
    pub state: String,
    pub proof: String,
}
#[derive(Serialize, Deserialize)]
struct Credential {
    token: String,
    nonce: String,
    subject: String,
}
impl OidcProvider {
    /// Discovers Authelia metadata/JWKS with redirects disabled and a bounded network deadline.
    pub async fn discover(
        issuer: String,
        client_id: String,
        client_secret: String,
        redirect: String,
    ) -> Result<Self, ProviderError> {
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(/*secs*/ 15))
            .build()
            .map_err(|_| ProviderError::Unavailable)?;
        Self::discover_with_http(issuer, client_id, client_secret, redirect, http).await
    }
    /// Allows isolated contract tests to supply their fixture CA and local DNS mapping.
    pub(crate) async fn discover_with_http(
        issuer: String,
        client_id: String,
        client_secret: String,
        redirect: String,
        http: reqwest::Client,
    ) -> Result<Self, ProviderError> {
        let issuer_url = IssuerUrl::new(issuer.clone()).map_err(|_| ProviderError::Rejected)?;
        let metadata = CoreProviderMetadata::discover_async(issuer_url, &http)
            .await
            .map_err(|_| ProviderError::Unavailable)?;
        let client = CoreClient::from_provider_metadata(
            metadata,
            ClientId::new(client_id.clone()),
            Some(ClientSecret::new(client_secret.clone())),
        )
        .set_redirect_uri(RedirectUrl::new(redirect).map_err(|_| ProviderError::Rejected)?);
        // Authelia's revocation endpoint is provider-owned configuration, never request input.
        let revocation_url = url::Url::parse(&format!(
            "{}/api/oidc/revocation",
            issuer.trim_end_matches('/')
        ))
        .map_err(|_| ProviderError::Rejected)?;
        Ok(Self {
            client,
            http,
            issuer,
            client_id,
            client_secret,
            revocation_url,
        })
    }
    /// Requests email and refresh capability with random state, nonce and S256 PKCE.
    pub fn authorize(&self) -> Result<LoginRedirect, ProviderError> {
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state, nonce) = self
            .client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("email".into()))
            .add_scope(Scope::new("profile".into()))
            .add_scope(Scope::new("offline_access".into()))
            .set_pkce_challenge(challenge)
            .url();
        let proof = serde_json::to_string(&LoginProof {
            nonce: nonce.secret().clone(),
            verifier: verifier.secret().clone(),
        })
        .map_err(|_| ProviderError::Rejected)?;
        Ok(LoginRedirect {
            url,
            state: state.secret().clone(),
            proof,
        })
    }
    /// Exchanges a browser-bound, already consumed state and verifies every ID-token security claim.
    pub async fn callback(
        &self,
        code: String,
        proof: &str,
    ) -> Result<IdentityTokens, ProviderError> {
        let proof: LoginProof = serde_json::from_str(proof).map_err(|_| ProviderError::Rejected)?;
        let response = self
            .client
            .exchange_code(AuthorizationCode::new(code))
            .map_err(|_| ProviderError::Rejected)?
            .set_pkce_verifier(PkceCodeVerifier::new(proof.verifier))
            .request_async(&|request| Self::request(self.http.clone(), request))
            .await
            .map_err(classify_token_error)?;
        let token = response.id_token().ok_or(ProviderError::Rejected)?;
        let verifier = self.client.id_token_verifier();
        let claims = token
            .claims(&verifier, &Nonce::new(proof.nonce.clone()))
            .map_err(|_| ProviderError::Rejected)?;
        self.check_access_hash(&response)?;
        let email = self.current_email(&response, claims.subject()).await?;
        let subject = claims.subject().as_str().to_owned();
        let refresh = response
            .refresh_token()
            .ok_or(ProviderError::Rejected)?
            .secret()
            .clone();
        let credential = Credential {
            token: refresh,
            nonce: proof.nonce,
            subject: subject.clone(),
        };
        Ok(IdentityTokens {
            issuer: self.issuer.clone(),
            subject,
            email,
            refresh: serde_json::to_string(&credential).map_err(|_| ProviderError::Rejected)?,
        })
    }
    /// Reads current email from subject-bound UserInfo; Authelia need not include profile claims in ID tokens.
    async fn current_email(
        &self,
        response: &CoreTokenResponse,
        subject: &openidconnect::SubjectIdentifier,
    ) -> Result<String, ProviderError> {
        let info: openidconnect::core::CoreUserInfoClaims = self
            .client
            .user_info(response.access_token().clone(), Some(subject.clone()))
            .map_err(|_| ProviderError::Rejected)?
            .request_async(&|request| Self::request(self.http.clone(), request))
            .await
            .map_err(|error| match error {
                openidconnect::UserInfoError::Request(_) => ProviderError::Unavailable,
                _ => ProviderError::Rejected,
            })?;
        Ok(info
            .email()
            .ok_or(ProviderError::Rejected)?
            .as_str()
            .to_owned())
    }
    /// Treats HTTP server outages as transport failures so revalidation never revokes on a 5xx.
    async fn request(
        http: reqwest::Client,
        request: openidconnect::HttpRequest,
    ) -> Result<openidconnect::HttpResponse, ProviderError> {
        let response = openidconnect::AsyncHttpClient::call(&http, request)
            .await
            .map_err(|_| ProviderError::Unavailable)?;
        if response.status().is_server_error()
            || response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
        {
            return Err(ProviderError::Unavailable);
        }
        Ok(response)
    }
    /// Binds an ID token's optional at_hash to the access token returned by the same exchange.
    fn check_access_hash(&self, response: &CoreTokenResponse) -> Result<(), ProviderError> {
        let token = response.id_token().ok_or(ProviderError::Rejected)?;
        let verifier = self.client.id_token_verifier();
        // Signature/issuer/audience/expiry are checked again; nonce is checked at the flow-specific callsite.
        let claims = token
            .claims(&verifier, |_: Option<&Nonce>| Ok(()))
            .map_err(|_| ProviderError::Rejected)?;
        if let Some(expected) = claims.access_token_hash() {
            let actual = AccessTokenHash::from_token(
                response.access_token(),
                token.signing_alg().map_err(|_| ProviderError::Rejected)?,
                token
                    .signing_key(&verifier)
                    .map_err(|_| ProviderError::Rejected)?,
            )
            .map_err(|_| ProviderError::Rejected)?;
            if actual != *expected {
                return Err(ProviderError::Rejected);
            }
        }
        Ok(())
    }
}
impl IdentityProvider for OidcProvider {
    /// Refreshes short-lived tokens, verifies the same subject and fetches current UserInfo email.
    async fn refresh(&self, credential: &str) -> Result<IdentityTokens, ProviderError> {
        let stored: Credential =
            serde_json::from_str(credential).map_err(|_| ProviderError::Rejected)?;
        let response = self
            .client
            .exchange_refresh_token(&RefreshToken::new(stored.token.clone()))
            .map_err(|_| ProviderError::Rejected)?
            .request_async(&|request| Self::request(self.http.clone(), request))
            .await
            .map_err(classify_token_error)?;
        let token = response.id_token().ok_or(ProviderError::Rejected)?;
        let verifier = self.client.id_token_verifier();
        let claims = token
            .claims(&verifier, |nonce: Option<&Nonce>| {
                if nonce.is_some_and(|nonce| nonce.secret() != &stored.nonce) {
                    Err("refresh nonce mismatch".into())
                } else {
                    Ok(())
                }
            })
            .map_err(|_| ProviderError::Rejected)?;
        if claims.subject().as_str() != stored.subject {
            return Err(ProviderError::Rejected);
        }
        self.check_access_hash(&response)?;
        let email = self.current_email(&response, claims.subject()).await?;
        let next = response
            .refresh_token()
            .map_or(stored.token, |token| token.secret().clone());
        let credential = Credential {
            token: next,
            nonce: stored.nonce,
            subject: stored.subject.clone(),
        };
        Ok(IdentityTokens {
            issuer: self.issuer.clone(),
            subject: stored.subject,
            email,
            refresh: serde_json::to_string(&credential).map_err(|_| ProviderError::Rejected)?,
        })
    }
    /// Revokes refresh material without exposing access or identity tokens to the browser.
    async fn revoke(&self, credential: &str) -> Result<(), ProviderError> {
        let stored: Credential =
            serde_json::from_str(credential).map_err(|_| ProviderError::Rejected)?;
        let response = self
            .http
            .post(self.revocation_url.clone())
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .form(&[
                ("token", stored.token.as_str()),
                ("token_type_hint", "refresh_token"),
            ])
            .send()
            .await
            .map_err(|_| ProviderError::Unavailable)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(ProviderError::Unavailable)
        }
    }
}
/// Separates retryable transport failures from deterministic OAuth/protocol rejection.
fn classify_token_error<RE: std::error::Error + 'static, TE: openidconnect::ErrorResponse>(
    error: openidconnect::RequestTokenError<RE, TE>,
) -> ProviderError {
    match error {
        openidconnect::RequestTokenError::Request(_) => ProviderError::Unavailable,
        _ => ProviderError::Rejected,
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod contract;
