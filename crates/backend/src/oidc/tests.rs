use super::*;
use axum::{
    Json, Router,
    routing::{get, post},
};
use openidconnect::{
    AccessToken, Audience, EmptyAdditionalClaims, EndUserEmail, JsonWebKeyId, JsonWebKeySet,
    PrivateSigningKey, StandardClaims, SubjectIdentifier,
    core::{CoreIdToken, CoreIdTokenClaims, CoreJwsSigningAlgorithm, CoreRsaPrivateSigningKey},
};
use pretty_assertions::assert_eq;

/// Exercises actual token signature and claim verification against a local fake token endpoint.
#[tokio::test]
async fn callback_rejects_invalid_signed_claims_and_requests_pkce() {
    let signing = CoreRsaPrivateSigningKey::from_pem(
        include_str!("test-key.pem"),
        Some(JsonWebKeyId::new("test".into())),
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let issuer = format!("http://{}", listener.local_addr().unwrap());
    let response = std::sync::Arc::new(std::sync::Mutex::new(serde_json::Value::Null));
    let state = response.clone();
    let app = Router::new().route(
        "/token",
        post(move |body: String| {
            let state = state.clone();
            async move {
                assert!(body.contains("code_verifier="));
                Json(state.lock().unwrap().clone())
            }
        }),
    );
    let info_state = response.clone();
    let app = app.route(
        "/userinfo",
        get(move || {
            let state = info_state.clone();
            async move {
                let value = state.lock().unwrap();
                Json(serde_json::json!({"sub":"subject","email":value["test_email"]}))
            }
        }),
    );
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let metadata:CoreProviderMetadata=serde_json::from_value(serde_json::json!({"issuer":issuer,"authorization_endpoint":format!("{issuer}/authorize"),"token_endpoint":format!("{issuer}/token"),"userinfo_endpoint":format!("{issuer}/userinfo"),"jwks_uri":format!("{issuer}/jwks"),"response_types_supported":["code"],"subject_types_supported":["public"],"id_token_signing_alg_values_supported":["RS256"]})).unwrap();
    let client = CoreClient::from_provider_metadata(
        metadata.set_jwks(JsonWebKeySet::new(vec![signing.as_verification_key()])),
        ClientId::new("palace".into()),
        Some(ClientSecret::new("secret".into())),
    );
    let provider = OidcProvider {
        client,
        http: reqwest::Client::new(),
        issuer: issuer.clone(),
        client_id: "palace".into(),
        client_secret: "secret".into(),
        revocation_url: url::Url::parse(&format!("{issuer}/revoke")).unwrap(),
    };
    let login = provider.authorize().unwrap();
    let query: std::collections::HashMap<_, _> = login.url.query_pairs().into_owned().collect();
    assert_eq!(query.get("code_challenge_method").unwrap(), "S256");
    assert_eq!(query.get("state").unwrap(), &login.state);
    let proof: LoginProof = serde_json::from_str(&login.proof).unwrap();
    palace_logging::initialize_test_clock();
    let now =
        chrono::DateTime::from_timestamp(palace_logging::clock::now_local().unix_timestamp(), 0)
            .unwrap();
    for invalid in [
        "none",
        "issuer",
        "audience",
        "expiry",
        "nonce",
        "email",
        "signature",
        "access_hash",
    ] {
        let token_issuer = if invalid == "issuer" {
            "https://other"
        } else {
            &issuer
        };
        let audience = if invalid == "audience" {
            "other"
        } else {
            "palace"
        };
        let expiry = if invalid == "expiry" {
            now - chrono::Duration::seconds(300)
        } else {
            now + chrono::Duration::seconds(300)
        };
        let nonce = if invalid == "nonce" {
            "wrong"
        } else {
            &proof.nonce
        };
        let standard = StandardClaims::new(SubjectIdentifier::new("subject".into())).set_email(
            if invalid == "email" {
                None
            } else {
                Some(EndUserEmail::new("a@example.com".into()))
            },
        );
        let claims = CoreIdTokenClaims::new(
            IssuerUrl::new(token_issuer.into()).unwrap(),
            vec![Audience::new(audience.into())],
            expiry,
            now,
            standard,
            EmptyAdditionalClaims {},
        )
        .set_nonce(Some(Nonce::new(nonce.into())));
        let access = AccessToken::new("access".into());
        let jwt = CoreIdToken::new(
            claims,
            &signing,
            CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
            Some(&access),
            /*code*/ None,
        )
        .unwrap();
        let mut jwt = jwt.to_string();
        if invalid == "signature" {
            let index = jwt.rfind('.').unwrap() + 1;
            jwt.replace_range(
                index..index + 1,
                if &jwt[index..index + 1] == "A" {
                    "B"
                } else {
                    "A"
                },
            );
        }
        *response.lock().unwrap() = serde_json::json!({"access_token":if invalid=="access_hash"{"substituted"}else{"access"},"test_email":if invalid=="email"{serde_json::Value::Null}else{serde_json::json!("a@example.com")},"token_type":"Bearer","refresh_token":"refresh","expires_in":300,"id_token":jwt});
        let result = provider.callback("code".into(), &login.proof).await;
        if invalid == "none" {
            let result = result.unwrap();
            assert_eq!(
                (result.issuer, result.subject, result.email),
                (issuer.clone(), "subject".into(), "a@example.com".into())
            );
        } else {
            assert!(matches!(result, Err(ProviderError::Rejected)), "{invalid}");
        }
    }
    server.abort();
}
