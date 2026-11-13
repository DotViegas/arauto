-- Arauto hub: the panel of the admin (docs/decisoes/0040-painel-dentro-do-hub.md).
--
-- The rules of 0001 hold here too. What this migration adds to them:
--   * the entry code and the identifier of a session of the panel are kept only as their
--     SHA-256, and the digest is erased as soon as the code or the session stops being
--     accepted;
--   * a row is found from the outside only by its digest.

-- ---------------------------------------------------------------------------- the entry code

-- A code of one use, issued by the admin command on the machine of the hub, that opens one
-- session of the panel.
CREATE TABLE panel_entry (
    id                  UUID PRIMARY KEY,
    organization_id     UUID   NOT NULL REFERENCES organization (id),
    -- SHA-256 of the entry code. Never the code.
    digest              BYTEA  CHECK (digest IS NULL OR octet_length(digest) = 32),
    state               TEXT   NOT NULL CHECK (state IN ('issued', 'used', 'cancelled')),
    created_by          UUID   NOT NULL REFERENCES admin (id),
    created_at          BIGINT NOT NULL,
    expires_at          BIGINT NOT NULL,
    used_at             BIGINT,
    CHECK ((state = 'issued') = (digest IS NOT NULL)),
    CHECK ((state = 'used') = (used_at IS NOT NULL))
);
CREATE UNIQUE INDEX panel_entry_digest ON panel_entry (digest) WHERE digest IS NOT NULL;
-- One code at a time: issuing another cancels the one before.
CREATE UNIQUE INDEX panel_entry_one_issued ON panel_entry (organization_id) WHERE state = 'issued';

-- ---------------------------------------------------------------------------- the session of the panel

-- The login of an admin in a browser. How long it has been idle is kept in the memory of the
-- hub, so that a request that only reads writes nothing here.
CREATE TABLE panel_session (
    id                  UUID PRIMARY KEY,
    organization_id     UUID   NOT NULL REFERENCES organization (id),
    admin_id            UUID   NOT NULL REFERENCES admin (id),
    -- SHA-256 of the identifier that goes in the cookie. Never the identifier.
    digest              BYTEA  CHECK (digest IS NULL OR octet_length(digest) = 32),
    state               TEXT   NOT NULL CHECK (state IN ('open', 'closed')),
    -- Why it closed: the admin signed out, it passed its hours, or the hub stopped.
    close_reason        TEXT   CHECK (close_reason IN ('signed_out', 'expired', 'hub_stopped')),
    opened_from         TEXT,
    created_at          BIGINT NOT NULL,
    expires_at          BIGINT NOT NULL,
    closed_at           BIGINT,
    CHECK ((state = 'open') = (digest IS NOT NULL)),
    CHECK ((state = 'closed') = (closed_at IS NOT NULL)),
    CHECK ((state = 'closed') = (close_reason IS NOT NULL))
);
CREATE UNIQUE INDEX panel_session_digest ON panel_session (digest) WHERE digest IS NOT NULL;
CREATE INDEX panel_session_open ON panel_session (organization_id) WHERE state = 'open';

-- ---------------------------------------------------------------------------- retention

-- A code and a session are dead by their `expires_at`. The purge takes their rows these days
-- after it; what each one did stays in the audit.
INSERT INTO retention (table_name, days) VALUES ('panel_entry', 30), ('panel_session', 30);
