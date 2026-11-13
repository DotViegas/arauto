-- Arauto hub: the passkeys of the panel (docs/decisoes/0019-painel-com-passkey.md and
-- docs/decisoes/0040-painel-dentro-do-hub.md).
--
-- The rules of 0001 and of 0013 hold here too. What this migration adds to them:
--   * a passkey is kept as what checks it (the public key and its counters), never as
--     something that could sign in its place;
--   * a session of the panel says how far it reaches: one opened by a code only reaches the
--     registration of a passkey; one opened by a passkey, or in which a passkey was
--     registered, reaches the whole panel.

-- ---------------------------------------------------------------------------- the passkey

CREATE TABLE panel_passkey (
    id                  UUID PRIMARY KEY,
    organization_id     UUID   NOT NULL REFERENCES organization (id),
    admin_id            UUID   NOT NULL REFERENCES admin (id),
    -- What the authenticator calls the credential. A browser sends it back to sign in.
    credential_id       BYTEA  NOT NULL CHECK (octet_length(credential_id) BETWEEN 1 AND 1023),
    -- What the authenticator calls the user: the same for every passkey of one admin.
    user_handle         BYTEA  NOT NULL CHECK (octet_length(user_handle) BETWEEN 1 AND 64),
    -- The public key and what never changes about the credential, as the library encodes it.
    static_state        BYTEA  NOT NULL,
    -- The counter of signatures and what may change at each use, as the library encodes it.
    dynamic_state       BYTEA  NOT NULL,
    state               TEXT   NOT NULL CHECK (state IN ('active', 'removed')),
    created_at          BIGINT NOT NULL,
    last_used_at        BIGINT,
    removed_at          BIGINT,
    CHECK ((state = 'removed') = (removed_at IS NOT NULL))
);
-- A credential belongs to one row, removed or not: an authenticator never repeats one.
CREATE UNIQUE INDEX panel_passkey_credential ON panel_passkey (credential_id);
CREATE INDEX panel_passkey_active ON panel_passkey (organization_id, admin_id) WHERE state = 'active';

-- ---------------------------------------------------------------------------- how far a session reaches

-- 'setup': opened by a code; it only reaches the registration of a passkey.
-- 'full': the person showed a passkey in it, signing in or registering one.
-- The sessions that exist were opened by a code: they are 'setup'. They are closed anyway
-- when the hub starts.
ALTER TABLE panel_session
    ADD COLUMN reach TEXT NOT NULL DEFAULT 'setup' CHECK (reach IN ('setup', 'full'));
