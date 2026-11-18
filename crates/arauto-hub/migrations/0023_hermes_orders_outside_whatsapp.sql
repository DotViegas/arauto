-- A Hermes whose orders come from outside WhatsApp, as the admin recorded it (decision 0051,
-- proposal, 18/11/2026). It refuses nothing: it answers the question of who gives the Hermes
-- orders on WhatsApp with "nobody", so the warnings stop. The hub cannot check it. The row
-- exists only while the Hermes has no row in hermes_whatsapp_phone: the one write path
-- (repo::orders::set) deletes both and writes one, under the lock of the Hermes row.
CREATE TABLE hermes_orders_outside_whatsapp (
    organization_id UUID   NOT NULL,
    hermes_id       UUID   NOT NULL,
    created_at      BIGINT NOT NULL,
    PRIMARY KEY (organization_id, hermes_id),
    FOREIGN KEY (organization_id, hermes_id) REFERENCES hermes (organization_id, id)
);
