use super::*;
use pretty_assertions::assert_eq;
use testcontainers::{
    GenericImage, ImageExt,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};

/// Exercises discovery, a real login, code exchange, current UserInfo, refresh and external revocation.
/// Core test case:
/// - `specs/test-cases/server/owner/owner-isolation.md#an-inactive-session-past-24-hours-must-revalidate-on-its-next-request`
#[tokio::test]
#[ignore = "requires prepared authelia/authelia:4.39.20 image and Docker/Podman socket"]
async fn authelia_authorization_refresh_and_revocation_contract() {
    assert!(
        std::process::Command::new("docker")
            .args(["image", "inspect", "authelia/authelia:4.39.20"])
            .output()
            .unwrap()
            .status
            .success(),
        "prepare Authelia 4.39.20; downloads are forbidden"
    );
    let reservation = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let signer = include_str!("test-key.pem")
        .lines()
        .map(|line| format!("          {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let config = include_str!("../../tests/fixtures/authelia/configuration.yml")
        .replace("@PORT@", &port.to_string())
        .replace("@SIGNING_KEY@", &signer);
    let container = GenericImage::new("authelia/authelia", "4.39.20")
        .with_exposed_port(9091.tcp())
        .with_wait_for(WaitFor::message_on_stdout("Listening for TLS connections"))
        .with_mapped_port(port, 9091.tcp())
        .with_copy_to("/config/configuration.yml", config.into_bytes())
        .with_copy_to(
            "/config/users.yml",
            include_bytes!("../../tests/fixtures/authelia/users.yml").to_vec(),
        )
        .with_copy_to(
            "/config/tls-cert.pem",
            include_bytes!("../../tests/fixtures/authelia/tls-cert.pem").to_vec(),
        )
        .with_copy_to(
            "/config/tls-key.pem",
            include_bytes!("../../tests/fixtures/authelia/tls-key.pem").to_vec(),
        )
        .start()
        .await
        .unwrap();
    let issuer = format!("https://auth.palace.test:{port}");
    let cert = reqwest::Certificate::from_pem(include_bytes!(
        "../../tests/fixtures/authelia/tls-cert.pem"
    ))
    .unwrap();
    let http = reqwest::Client::builder()
        .no_proxy()
        .add_root_certificate(cert)
        .resolve("auth.palace.test", ([127, 0, 0, 1], port).into())
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(/*secs*/ 15))
        .build()
        .unwrap();
    let provider = OidcProvider::discover_with_http(
        issuer.clone(),
        "palace".into(),
        "palace-test-client-secret".into(),
        "https://palace.test/auth/callback".into(),
        http.clone(),
    )
    .await
    .unwrap();
    let login = provider.authorize().unwrap();
    let response=http.post(format!("{issuer}/api/firstfactor")).header("Origin",&issuer).json(&serde_json::json!({"username":"eric-test","password":"palace-test-password","keepMeLoggedIn":true,"targetURL":login.url.as_str(),"requestMethod":"GET"})).send().await.unwrap();
    let status = response.status();
    let cookies = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .map(|value| {
            value
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("; ");
    let body = response.text().await.unwrap();
    assert!(status.is_success(), "first factor: {status} {body}");
    assert!(
        !cookies.is_empty(),
        "expected Authelia session cookie: {body}"
    );
    let response = http
        .get(login.url.clone())
        .header("cookie", &cookies)
        .send()
        .await
        .unwrap();
    let status = response.status();
    let location = response
        .headers()
        .get("location")
        .map(|value| value.to_str().unwrap().to_owned());
    let body = response.text().await.unwrap();
    assert!(status.is_redirection(), "authorize: {status} {body}");
    let mut location = url::Url::parse(&location.unwrap()).unwrap();
    if location.path().starts_with("/consent/") {
        let flow = location
            .query_pairs()
            .find(|(name, _)| name == "flow_id")
            .unwrap()
            .1
            .into_owned();
        let consent: serde_json::Value = http
            .get(format!("{issuer}/api/oidc/consent"))
            .query(&[("flow_id", &flow)])
            .header("cookie", &cookies)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(consent["status"], "OK");
        let accepted: serde_json::Value = http.post(format!("{issuer}/api/oidc/consent")).header("cookie",&cookies).header("Origin",&issuer).json(&serde_json::json!({"flow_id":flow,"client_id":"palace","consent":true,"pre_configure":false,"claims":consent["data"]["claims"].as_array().cloned().unwrap_or_default()})).send().await.unwrap().json().await.unwrap();
        assert_eq!(accepted["status"], "OK", "consent: {accepted}");
        location = url::Url::parse(accepted["data"]["redirect_uri"].as_str().unwrap()).unwrap();
        if location.host_str() == Some("auth.palace.test") {
            let response = http
                .get(location)
                .header("cookie", &cookies)
                .send()
                .await
                .unwrap();
            location = url::Url::parse(response.headers()["location"].to_str().unwrap()).unwrap();
        }
    }
    let query: std::collections::HashMap<_, _> = location.query_pairs().into_owned().collect();
    assert_eq!(
        query.get("state"),
        Some(&login.state),
        "authorization location: {location}; login body: {body}"
    );
    assert!(
        query.contains_key("code"),
        "authorization response: {location}"
    );
    let tokens = provider
        .callback(query["code"].clone(), &login.proof)
        .await
        .unwrap();
    assert_eq!(
        (&tokens.issuer, tokens.email.as_str()),
        (&issuer, "eric-test@example.com")
    );
    let refreshed = provider.refresh(&tokens.refresh).await.unwrap();
    assert_eq!(
        (&refreshed.issuer, &refreshed.subject, &refreshed.email),
        (&tokens.issuer, &tokens.subject, &tokens.email)
    );
    provider.revoke(&refreshed.refresh).await.unwrap();
    assert!(matches!(
        provider.refresh(&refreshed.refresh).await,
        Err(ProviderError::Rejected)
    ));
    drop(container);
}
