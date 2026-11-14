-- Arauto hub: the first access of the panel (docs/decisoes/0040-painel-dentro-do-hub.md).
--
-- The rules of 0001, 0013 and 0014 hold here too. What this migration adds to them:
--   * an installation code has a kind: 'start' is the one the hub writes in its own log when
--     it starts before the first access, and it has no deadline; 'command' is the one the
--     admin command issues, and it is valid for minutes. The database enforces that a code
--     has a deadline if, and only if, it is a 'command' one;
--   * a cancelled code says when it was cancelled and by which kind of code;
--   * a session of the panel opened by a code points at that code, and typing a code does not
--     spend it: registering a passkey in that session does;
--   * "the first access was done" is a durable mark on the organization, written in the same
--     transaction that keeps the first passkey. It is never cleared: losing every passkey
--     later does not bring the start code back.

-- ---------------------------------------------------------------------------- the kinds of code

-- The rows that exist were issued by the command, and they keep the deadline they have:
-- the ones that are dead stay dead.
ALTER TABLE panel_entry ADD COLUMN kind TEXT NOT NULL DEFAULT 'command'
    CHECK (kind IN ('start', 'command'));
ALTER TABLE panel_entry ALTER COLUMN kind DROP DEFAULT;
ALTER TABLE panel_entry ALTER COLUMN expires_at DROP NOT NULL;
ALTER TABLE panel_entry ADD CONSTRAINT panel_entry_deadline_of_kind
    CHECK ((kind = 'start') = (expires_at IS NULL));

-- When a code was cancelled and the kind of the code that cancelled it. A cancelled row that
-- exists already is counted from its deadline, as the purge did.
ALTER TABLE panel_entry ADD COLUMN cancelled_at BIGINT;
ALTER TABLE panel_entry ADD COLUMN cancelled_by TEXT CHECK (cancelled_by IN ('start', 'command'));
UPDATE panel_entry SET cancelled_at = expires_at, cancelled_by = 'command' WHERE state = 'cancelled';
ALTER TABLE panel_entry ADD CONSTRAINT panel_entry_cancelled_has_its_facts
    CHECK ((state = 'cancelled') = (cancelled_at IS NOT NULL)
       AND (state = 'cancelled') = (cancelled_by IS NOT NULL));

-- ---------------------------------------------------------------------------- the session of a code

-- The code that opened a session, until the purge takes the code.
ALTER TABLE panel_session ADD COLUMN entry_id UUID REFERENCES panel_entry (id) ON DELETE SET NULL;
CREATE INDEX panel_session_entry ON panel_session (entry_id) WHERE entry_id IS NOT NULL;

-- Three more ways a session ends: the code that opened it was replaced by another; the code
-- was typed again and the session of that typing took its place; or the code was spent
-- elsewhere, or is gone, when a passkey came to be registered in the session.
ALTER TABLE panel_session DROP CONSTRAINT panel_session_close_reason_check;
ALTER TABLE panel_session ADD CONSTRAINT panel_session_close_reason_check
    CHECK (close_reason IN ('signed_out', 'expired', 'hub_stopped', 'code_replaced', 'code_retyped',
                            'code_spent'));

-- ---------------------------------------------------------------------------- the durable mark

-- When the first passkey of the organization was kept. Null until then.
ALTER TABLE organization ADD COLUMN panel_first_passkey_at BIGINT;
-- An installation that has a passkey already did its first access, removed or not.
UPDATE organization o SET panel_first_passkey_at =
    (SELECT min(p.created_at) FROM panel_passkey p WHERE p.organization_id = o.id);

-- Once written, the mark is never cleared nor changed.
CREATE FUNCTION panel_first_passkey_is_kept() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.panel_first_passkey_at IS NOT NULL
       AND NEW.panel_first_passkey_at IS DISTINCT FROM OLD.panel_first_passkey_at THEN
        RAISE EXCEPTION 'the mark of the first access is never cleared';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER organization_first_passkey_kept BEFORE UPDATE ON organization
    FOR EACH ROW EXECUTE FUNCTION panel_first_passkey_is_kept();
