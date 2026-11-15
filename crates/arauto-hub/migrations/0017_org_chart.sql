-- Arauto hub: the org chart of the panel (docs/decisoes/0043-organograma-e-niveis.md).
-- Stage A: what the admin sees and arranges. No table here is read by the channel, the
-- notices or the tools; the kind of an arrow is not stored: the panel computes it.
-- The rules of 0001 hold: time is BIGINT ms from the hub, ids are UUIDs drawn by the hub,
-- every row carries organization_id.

CREATE TABLE org_level (
    id                  UUID    PRIMARY KEY,
    organization_id     UUID    NOT NULL REFERENCES organization (id),
    -- 0 is the top. The hub writes 0..n-1 with no gap.
    position            INTEGER NOT NULL CHECK (position BETWEEN 0 AND 11),
    -- The format and the reserved words of a Hermes name.
    name                TEXT    NOT NULL CHECK (
                            name ~ '^[a-z0-9][a-z0-9_-]{0,31}$'
                            AND name NOT IN ('system', 'operator', 'admin', 'arauto', 'user',
                                             'assistant', 'hub', 'hermes', 'developer', 'tool')),
    created_at          BIGINT  NOT NULL,
    changed_at          BIGINT  NOT NULL,
    UNIQUE (organization_id, id),
    -- Deferred: one publish may swap two names or two positions.
    CONSTRAINT org_level_name_once UNIQUE (organization_id, name) DEFERRABLE INITIALLY DEFERRED,
    CONSTRAINT org_level_position_once UNIQUE (organization_id, position) DEFERRABLE INITIALLY DEFERRED
);

-- Where a Hermes sits. One row per Hermes at most: a Hermes is in one level only.
-- A live Hermes with no row is "not placed yet".
CREATE TABLE org_member (
    hermes_id           UUID    PRIMARY KEY,
    organization_id     UUID    NOT NULL,
    level_id            UUID    NOT NULL,
    -- Left edge of the card on the map, in CSS pixels of the map.
    map_x               INTEGER NOT NULL CHECK (map_x BETWEEN 0 AND 4000),
    -- For the admin's own reading. NULL when there is none. No control, line separator or
    -- direction character (written out, so that no locale changes what it means).
    role                TEXT    CHECK (role IS NULL OR (
                            char_length(role) BETWEEN 1 AND 280
                            AND role !~ '[\u0001-\u001f\u007f-\u009f\u2028\u2029\u202a-\u202e\u2066-\u2069\U000e0000-\U000e007f]')),
    changed_at          BIGINT  NOT NULL,
    FOREIGN KEY (organization_id, hermes_id) REFERENCES hermes (organization_id, id),
    FOREIGN KEY (organization_id, level_id) REFERENCES org_level (organization_id, id)
);
CREATE INDEX org_member_level ON org_member (level_id);
