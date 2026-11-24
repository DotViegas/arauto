-- Arauto hub: the short-lived address of the file of a message (decision 0055, proposal,
-- 24/11/2026).
--
-- media_link: one address `whatsapp_media_link` gave a Hermes. The address is a short-lived
-- secret: only its SHA-256 is kept (token_digest), never the address. The message is the
-- identifier the engine knows it by, sealed with the key of the organization and bound to this
-- row (message_enc). A GET or HEAD of the address reads this row and writes nothing: the row
-- ends only at expires_at, and the purge removes it one day after.
CREATE TABLE media_link (
    id                  UUID PRIMARY KEY,
    organization_id     UUID   NOT NULL,
    number_id           UUID   NOT NULL,
    chat_id             UUID   NOT NULL,
    hermes_id           UUID   NOT NULL,
    message_enc         BYTEA  NOT NULL,
    token_digest        BYTEA  NOT NULL UNIQUE CHECK (octet_length(token_digest) = 32),
    created_at          BIGINT NOT NULL,
    expires_at          BIGINT NOT NULL CHECK (expires_at > created_at),
    FOREIGN KEY (organization_id, chat_id) REFERENCES chat (organization_id, id),
    FOREIGN KEY (organization_id, number_id) REFERENCES number (organization_id, id),
    FOREIGN KEY (organization_id, hermes_id) REFERENCES hermes (organization_id, id) ON DELETE CASCADE
);
CREATE INDEX media_link_expires ON media_link (expires_at);

INSERT INTO retention (table_name, days) VALUES ('media_link', 1);
