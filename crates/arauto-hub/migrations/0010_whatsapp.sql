-- Arauto hub: the engine, the numbers, the chats and the inbox of engine events
-- (docs/06-modelo-de-dados.md, sections 4 and 6; docs/10-maquinas-de-estado.md, section 4).
--
-- The rules of 0001 hold here too. What this migration adds to them:
--   * the secrets of the engine (its API key and the secret that signs its events) are sealed
--     by the vault with the master key, in columns ending in `_enc`; never in clear;
--   * a telephone, an identifier of a chat in the engine and the body of an event are
--     personal data of third parties: each is encrypted with the key of the organization,
--     and where the hub must find a row by one of them it keeps a keyed digest (HMAC-SHA-256
--     with a key derived from the key of the organization), never the value;
--   * the matrix of accesses (access, access_chat) says what each Hermes may do on each number
--     and, line by line, in each chat (docs/08-permissoes.md); a line of a chat is always of
--     the number of its access, and the database refuses one that is not.

-- ---------------------------------------------------------------------------- engine

CREATE TABLE engine (
    id                  UUID PRIMARY KEY,
    organization_id     UUID   NOT NULL REFERENCES organization (id),
    kind                TEXT   NOT NULL CHECK (kind IN ('waha')),
    -- Where the hub reaches the engine: http://host[:port], on the private network.
    address             TEXT   NOT NULL CHECK (length(address) BETWEEN 8 AND 255),
    -- Where the engine reaches the internal port of the hub, without a path.
    events_url          TEXT   NOT NULL CHECK (length(events_url) BETWEEN 8 AND 255),
    -- Sealed by the vault with the master key. The key may be missing until the admin sets it.
    api_key_enc         BYTEA,
    events_secret_enc   BYTEA  NOT NULL,
    -- Grows on every change of the address, the key or the secret: the hub rebuilds its
    -- client, and reapplies the configuration of each session of the engine.
    config_version      BIGINT NOT NULL CHECK (config_version >= 1),
    version_seen        TEXT   CHECK (version_seen IS NULL OR version_seen ~ '^[A-Za-z0-9.+_-]{1,64}$'),
    version_tested      BOOLEAN,
    last_ok_at          BIGINT,
    created_at          BIGINT NOT NULL,
    changed_at          BIGINT NOT NULL,
    -- One engine per organization in the MVP (docs/03-arquitetura.md, section 5).
    UNIQUE (organization_id),
    UNIQUE (organization_id, id)
);

-- ---------------------------------------------------------------------------- number

CREATE TABLE number (
    id                  UUID PRIMARY KEY,
    organization_id     UUID   NOT NULL,
    engine_id           UUID   NOT NULL,
    -- Drawn by the hub, never reused, never given by a Hermes.
    session_name        TEXT   NOT NULL UNIQUE CHECK (session_name ~ '^[a-z][a-z0-9]{15,62}$'),
    phone_enc           BYTEA  NOT NULL,
    phone_digest        BYTEA  NOT NULL CHECK (octet_length(phone_digest) = 32),
    -- The same restricted format as the name of a Hermes; never reused in the organization.
    alias               TEXT   NOT NULL CHECK (alias ~ '^[a-z0-9][a-z0-9_-]{0,31}$'),
    command             BOOLEAN NOT NULL DEFAULT FALSE,
    state               TEXT   NOT NULL CHECK (state IN (
                            'unpaired', 'pairing', 'connected', 'reconnecting', 'disconnected',
                            'number_swapped', 'failed', 'removed')),
    state_since         BIGINT NOT NULL,
    -- The time, in the clock of the engine, of the last status of the session of the engine
    -- that was applied: an older one that arrives later is ignored.
    status_at           BIGINT,
    paired_at           BIGINT,
    pairing_until       BIGINT,
    -- The telephone that paired, when it is not the expected one: what the admin accepts or
    -- undoes.
    paired_phone_enc    BYTEA,
    paired_phone_digest BYTEA  CHECK (paired_phone_digest IS NULL OR octet_length(paired_phone_digest) = 32),
    -- SHA-256 of the configuration of the session of the engine the hub applied last.
    config_digest       BYTEA  CHECK (config_digest IS NULL OR octet_length(config_digest) = 32),
    config_applied_at   BIGINT,
    -- The newest message recorded, in milliseconds of the engine: where the catch-up starts.
    last_message_at     BIGINT,
    created_at          BIGINT NOT NULL,
    removed_at          BIGINT,
    FOREIGN KEY (organization_id, engine_id) REFERENCES engine (organization_id, id),
    UNIQUE (organization_id, id),
    UNIQUE (organization_id, alias),
    CHECK ((state = 'removed') = (removed_at IS NOT NULL)),
    CHECK ((state = 'pairing') = (pairing_until IS NOT NULL)),
    CHECK ((state = 'number_swapped') = (paired_phone_digest IS NOT NULL)),
    CHECK ((paired_phone_enc IS NULL) = (paired_phone_digest IS NULL))
);
-- A telephone is added once while its number exists.
CREATE UNIQUE INDEX number_phone ON number (organization_id, phone_digest) WHERE removed_at IS NULL;
CREATE INDEX number_of_engine ON number (engine_id) WHERE removed_at IS NULL;

-- ---------------------------------------------------------------------------- chat

CREATE TABLE chat (
    id                  UUID PRIMARY KEY,
    organization_id     UUID   NOT NULL,
    number_id           UUID   NOT NULL,
    kind                TEXT   NOT NULL CHECK (kind IN ('contact', 'group')),
    -- Written by whoever is on the other side: untrusted, and encrypted.
    shown_name_enc      BYTEA,
    last_message_at     BIGINT,
    created_at          BIGINT NOT NULL,
    FOREIGN KEY (organization_id, number_id) REFERENCES number (organization_id, id),
    UNIQUE (organization_id, id),
    -- What a line of a chat in the matrix points at: the chat and its number together.
    UNIQUE (number_id, id)
);
CREATE INDEX chat_of_number ON chat (number_id, last_message_at DESC);

-- A chat may show up in the engine under more than one identifier (a telephone and a hidden
-- identifier, for instance).
CREATE TABLE chat_engine_id (
    organization_id     UUID   NOT NULL,
    number_id           UUID   NOT NULL,
    chat_id             UUID   NOT NULL,
    id_digest           BYTEA  NOT NULL CHECK (octet_length(id_digest) = 32),
    id_enc              BYTEA  NOT NULL,
    created_at          BIGINT NOT NULL,
    PRIMARY KEY (number_id, id_digest),
    FOREIGN KEY (organization_id, chat_id) REFERENCES chat (organization_id, id),
    FOREIGN KEY (organization_id, number_id) REFERENCES number (organization_id, id)
);
CREATE INDEX chat_engine_id_chat ON chat_engine_id (chat_id);

-- ---------------------------------------------------------------------------- engine_event (inbox)

CREATE TABLE engine_event (
    id                  UUID PRIMARY KEY,
    organization_id     UUID   NOT NULL,
    engine_id           UUID   NOT NULL,
    number_id           UUID   NOT NULL,
    -- The keyed digest of what makes the event one: the message for a message, the event of
    -- the engine for the others. The same event twice is one row.
    key_digest          BYTEA  NOT NULL CHECK (octet_length(key_digest) = 32),
    kind                TEXT   NOT NULL CHECK (kind IN (
                            'session.status', 'message.any', 'message.reaction',
                            'message.edited', 'message.revoked')),
    source              TEXT   NOT NULL CHECK (source IN ('webhook', 'catch_up')),
    body_enc            BYTEA  NOT NULL,
    -- The time the engine gave the event, in milliseconds of its clock.
    engine_at           BIGINT NOT NULL,
    received_at         BIGINT NOT NULL,
    processed_at        BIGINT,
    FOREIGN KEY (organization_id, engine_id) REFERENCES engine (organization_id, id),
    FOREIGN KEY (organization_id, number_id) REFERENCES number (organization_id, id),
    UNIQUE (engine_id, key_digest)
);
-- What is still to be handled, oldest first: the queue of notices reads from here.
CREATE INDEX engine_event_open ON engine_event (organization_id, received_at) WHERE processed_at IS NULL;
CREATE INDEX engine_event_processed ON engine_event (processed_at) WHERE processed_at IS NOT NULL;

INSERT INTO retention (table_name, days) VALUES ('engine_event', 1);

-- ---------------------------------------------------------------------------- notices

-- 0001 left these for the WhatsApp tables.
ALTER TABLE notice ADD CONSTRAINT notice_number_of_organization
    FOREIGN KEY (organization_id, number_id) REFERENCES number (organization_id, id);
ALTER TABLE notice ADD CONSTRAINT notice_chat_of_organization
    FOREIGN KEY (organization_id, chat_id) REFERENCES chat (organization_id, id);

-- ---------------------------------------------------------------------------- the matrix of accesses

-- One per pair (Hermes, number). The verbs of the number: what holds in every chat without a
-- line of its own when the scope is 'all', and for what is of the number itself (its state,
-- its listings, the question about a telephone). With the scope 'list', only the chats that
-- have a line exist for the Hermes.
CREATE TABLE access (
    id                  UUID PRIMARY KEY,
    organization_id     UUID   NOT NULL,
    hermes_id           UUID   NOT NULL,
    number_id           UUID   NOT NULL,
    scope               TEXT   NOT NULL CHECK (scope IN ('all', 'list')),
    can_read            BOOLEAN NOT NULL,
    can_send            BOOLEAN NOT NULL,
    can_modify          BOOLEAN NOT NULL,
    can_wake            BOOLEAN NOT NULL,
    -- Writing first to someone who never talked to the number.
    can_start_chats     BOOLEAN NOT NULL,
    created_by          UUID   NOT NULL REFERENCES admin (id),
    created_at          BIGINT NOT NULL,
    changed_at          BIGINT NOT NULL,
    FOREIGN KEY (organization_id, hermes_id) REFERENCES hermes (organization_id, id),
    FOREIGN KEY (organization_id, number_id) REFERENCES number (organization_id, id),
    UNIQUE (hermes_id, number_id),
    UNIQUE (organization_id, id),
    UNIQUE (id, number_id),
    -- Being woken needs reading; starting a chat needs sending, and every chat in scope.
    CHECK (NOT can_wake OR can_read),
    CHECK (NOT can_start_chats OR (can_send AND scope = 'all'))
);
CREATE INDEX access_of_number ON access (number_id);

-- The line of a chat: when it exists, its verbs replace the verbs of the number in that chat,
-- whole. A line with every verb off blocks the chat.
CREATE TABLE access_chat (
    organization_id     UUID   NOT NULL,
    access_id           UUID   NOT NULL,
    number_id           UUID   NOT NULL,
    chat_id             UUID   NOT NULL,
    can_read            BOOLEAN NOT NULL,
    can_send            BOOLEAN NOT NULL,
    can_modify          BOOLEAN NOT NULL,
    can_wake            BOOLEAN NOT NULL,
    created_at          BIGINT NOT NULL,
    changed_at          BIGINT NOT NULL,
    PRIMARY KEY (access_id, chat_id),
    FOREIGN KEY (organization_id, access_id) REFERENCES access (organization_id, id) ON DELETE CASCADE,
    -- The chat is of the number of the access.
    FOREIGN KEY (access_id, number_id) REFERENCES access (id, number_id) ON DELETE CASCADE,
    FOREIGN KEY (number_id, chat_id) REFERENCES chat (number_id, id),
    FOREIGN KEY (organization_id, chat_id) REFERENCES chat (organization_id, id),
    CHECK (NOT can_wake OR can_read)
);
CREATE INDEX access_chat_of_chat ON access_chat (chat_id);
