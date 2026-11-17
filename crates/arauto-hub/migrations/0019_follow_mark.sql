-- Stage B of the org chart (decision 0050): the follow mark of a link. Off on every existing
-- link. Only the publish of the org chart sets it, on a link that goes from a higher level to a
-- lower one; the database only holds the part it can check alone: it needs the wake mark.
ALTER TABLE peer_link ADD COLUMN follow BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE peer_link ADD CONSTRAINT peer_link_follow_needs_wake CHECK (NOT follow OR wake);
