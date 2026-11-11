-- Arauto hub: what the reviews of the connection stage asked of the schema.
--
-- The rules of 0001 hold here too. The WhatsApp tables, which 0001 announced for this number,
-- come in a later one.

-- ---------------------------------------------------------------------------- unknown devices

-- An identity the database does not know, with a valid seal, is proof that the hub came back
-- from an older copy (C11). The listing is committed before the restored mode is turned on,
-- so the row itself says whether that proof is still to be handled: a hub that stops between
-- the two finds it again at the next presentation.
ALTER TABLE unknown_device ADD COLUMN proof_pending BOOLEAN NOT NULL DEFAULT FALSE;

-- ---------------------------------------------------------------------------- identifiers

-- The only sequence of the schema is the one of the audit. No row was ever written here:
-- the column is replaced, not converted.
ALTER TABLE tool_call DROP COLUMN id;
ALTER TABLE tool_call ADD COLUMN id UUID PRIMARY KEY;

-- ---------------------------------------------------------------------------- one organization per row

-- A row that points at a device is of the organization of that device: the database refuses
-- a connection or an invite of one organization about a device of another.
ALTER TABLE device ADD CONSTRAINT device_of_organization UNIQUE (organization_id, id);
ALTER TABLE channel_connection ADD CONSTRAINT channel_connection_device_of_organization
    FOREIGN KEY (organization_id, device_id) REFERENCES device (organization_id, id);
ALTER TABLE invite ADD CONSTRAINT invite_device_of_organization
    FOREIGN KEY (organization_id, device_id) REFERENCES device (organization_id, id);

-- ---------------------------------------------------------------------------- indexes

-- The rows a hub that stopped without warning left open, read once when it starts.
CREATE INDEX channel_connection_open ON channel_connection (organization_id, device_id)
    WHERE closed_at IS NULL;
-- The connections of a device that received a credential, by when they ended: the count
-- behind the alert of the swaps left unconfirmed.
CREATE INDEX channel_connection_issued ON channel_connection (device_id, closed_at)
    WHERE credential_issued_at IS NOT NULL;
-- "Was this alert already raised about this device?"
CREATE INDEX alert_device_kind ON alert (organization_id, device_id, kind, at);
-- The sweep of the deadlines, which runs for one organization.
CREATE INDEX device_open_of_organization ON device (organization_id, state)
    WHERE slot IS NOT NULL;
