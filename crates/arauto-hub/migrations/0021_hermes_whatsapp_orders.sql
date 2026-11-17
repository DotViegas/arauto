-- Who gives a Hermes orders on WhatsApp, recorded by the admin (decision 0051, proposal).
--
-- The hub refuses every Hermes writing in the chat where a Hermes takes orders, when one end of
-- that chat is a number of the hub. The record is a fact about the Hermes, in one direction:
-- the telephone its own WhatsApp bridge runs on ('takes_orders', at most one) and the
-- telephones that give it orders ('gives_orders'), or its own WhatsApp in the chat with itself
-- (the mode 'self_chat', no 'gives_orders' row).
--
-- A telephone is personal data: it is sealed, and found only by a keyed digest of each of its
-- forms (the same function as number.phone_digest, so the two compare).
CREATE TABLE hermes_whatsapp_phone (
    id              UUID PRIMARY KEY,
    organization_id UUID   NOT NULL,
    hermes_id       UUID   NOT NULL,
    -- 'takes_orders': the telephone the Hermes's own WhatsApp bridge runs on (one per Hermes);
    -- 'gives_orders': a telephone that gives that Hermes orders.
    role            TEXT   NOT NULL CHECK (role IN ('takes_orders', 'gives_orders')),
    -- Only on the 'takes_orders' row: how the bridge takes orders.
    whatsapp_mode   TEXT   CHECK (whatsapp_mode IN ('bot', 'self_chat')),
    -- Sealed, WhatsappField::HermesWhatsappPhone, bound to the row.
    phone_enc       BYTEA  NOT NULL,
    created_at      BIGINT NOT NULL,
    FOREIGN KEY (organization_id, hermes_id) REFERENCES hermes (organization_id, id),
    UNIQUE (organization_id, id),
    CHECK ((role = 'takes_orders') = (whatsapp_mode IS NOT NULL))
);
CREATE UNIQUE INDEX hermes_whatsapp_takes_orders ON hermes_whatsapp_phone (hermes_id)
    WHERE role = 'takes_orders';

CREATE TABLE hermes_whatsapp_phone_digest (
    organization_id UUID  NOT NULL,
    phone_id        UUID  NOT NULL,
    role            TEXT  NOT NULL CHECK (role IN ('takes_orders', 'gives_orders')),
    -- whatsapp_digest(Phone, organization, digits) of each form of the telephone.
    digest          BYTEA NOT NULL CHECK (octet_length(digest) = 32),
    PRIMARY KEY (phone_id, digest),
    FOREIGN KEY (organization_id, phone_id) REFERENCES hermes_whatsapp_phone (organization_id, id)
        ON DELETE CASCADE
);
CREATE INDEX hermes_whatsapp_digest_of ON hermes_whatsapp_phone_digest (organization_id, digest);
-- One Hermes per bridge telephone: a second bridge on one account is not modelled.
CREATE UNIQUE INDEX hermes_whatsapp_takes_orders_digest
    ON hermes_whatsapp_phone_digest (organization_id, digest) WHERE role = 'takes_orders';
