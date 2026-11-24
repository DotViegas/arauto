-- Arauto hub: one Hermes session per contact (decision 0054, proposal, 24/11/2026).
--
-- sent_message: the messages a Hermes sent through Arauto, so that a reading can say who sent
-- each one. The engine message id is kept only as a keyed digest, like chat_engine_id.id_digest,
-- and the Hermes goes to NULL when its row is deleted: the name of a Hermes is never given.
CREATE TABLE sent_message (
    id                  UUID PRIMARY KEY,
    organization_id     UUID   NOT NULL,
    number_id           UUID   NOT NULL,
    chat_id             UUID   NOT NULL,
    hermes_id           UUID   REFERENCES hermes (id) ON DELETE SET NULL,
    id_digest           BYTEA  NOT NULL CHECK (octet_length(id_digest) = 32),
    sent_at             BIGINT NOT NULL,
    purge_at            BIGINT NOT NULL,
    FOREIGN KEY (organization_id, chat_id) REFERENCES chat (organization_id, id),
    FOREIGN KEY (organization_id, number_id) REFERENCES number (organization_id, id)
);
CREATE UNIQUE INDEX sent_message_digest ON sent_message (number_id, id_digest);
CREATE INDEX sent_message_chat ON sent_message (chat_id);
CREATE INDEX sent_message_purge ON sent_message (purge_at);

INSERT INTO retention (table_name, days) VALUES ('sent_message', 30);

-- Consecutive failures of the engine while the hub asked for the telephone of a hidden
-- identifier to build the key of a notice: after 5 the hidden key is used. A number only.
ALTER TABLE notice ADD COLUMN engine_failures INTEGER NOT NULL DEFAULT 0 CHECK (engine_failures >= 0);
