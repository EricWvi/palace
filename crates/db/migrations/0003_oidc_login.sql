-- Pending login state is global security infrastructure: no owner exists until OIDC verification.
CREATE TABLE oidc_login (
    id uuid PRIMARY KEY,
    state_hash bytea NOT NULL UNIQUE,
    browser_hash bytea NOT NULL,
    encrypted_proof bytea NOT NULL,
    expires_at bigint NOT NULL
);
