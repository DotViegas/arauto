-- What a first installation against a real WhatsApp engine needs.

-- The catch-up has a cursor of its own. Before, it started from the newest message recorded,
-- which every new event moved forward: a message that arrived right after the hub came back
-- hid everything that had arrived while it was away. Only the catch-up writes this column,
-- and only after a whole round of the number.
ALTER TABLE number ADD COLUMN caught_up_until BIGINT;

-- A chat the engine names by a hidden identifier (`...@lid`) instead of a telephone. The hub
-- cannot tell whose it is, so it cannot tell whether a line the admin gave by telephone is
-- about this person. See `repo::access::chat_line`.
ALTER TABLE chat ADD COLUMN hidden_id BOOLEAN NOT NULL DEFAULT FALSE;

-- The catch-up reads again the last day of each chat, and tells what it already has by the
-- row of the event: the handled events are kept for longer than that day.
UPDATE retention SET days = 2 WHERE table_name = 'engine_event';
