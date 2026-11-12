-- The wait before a message of WhatsApp wakes a Hermes (docs/08-permissoes.md, section 4).
--
-- Someone who writes usually sends a few messages in a row. The notice of a chat waits this
-- long after the last message before it leaves, so the Hermes is woken once, with the whole
-- of what was written. A notice never waits more than a minute in all. Zero wakes at once.
ALTER TABLE access
    ADD COLUMN wake_wait_ms INTEGER NOT NULL DEFAULT 6000
        CHECK (wake_wait_ms BETWEEN 0 AND 60000);
