-- The engine brought by the installation (docs/decisoes/0052-a-instalacao-traz-o-motor.md).
-- Who set this row: an admin with the command ('admin'), or the hub from the address the
-- installation gives ('installation'). Only an 'installation' row has its key drawn by the hub.
-- Existing rows become 'admin', the safe side: no key is drawn and no file is written for them.
-- The default stays, so a row written without the column is an admin row, never an installation
-- one.
ALTER TABLE engine ADD COLUMN origin TEXT NOT NULL DEFAULT 'admin'
    CHECK (origin IN ('admin', 'installation'));
-- A row the installation brought always has the key the hub drew for it.
ALTER TABLE engine ADD CONSTRAINT engine_installation_has_key
    CHECK (origin <> 'installation' OR api_key_enc IS NOT NULL);
-- At most one engine of the installation in the whole installation: one key, one digest file.
CREATE UNIQUE INDEX engine_one_installation ON engine ((true)) WHERE origin = 'installation';
-- When the engine began refusing the key of the hub: one alert per episode, cleared by the first
-- answer that works or by a change of the configuration.
ALTER TABLE engine ADD COLUMN key_refused_since BIGINT;
