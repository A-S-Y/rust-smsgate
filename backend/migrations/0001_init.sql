CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT,
    encrypted BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE messages (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    direction TEXT NOT NULL CHECK (direction IN ('sent', 'received')),
    status TEXT NOT NULL,
    phone_number TEXT NOT NULL,
    message_content TEXT NOT NULL,
    message_id TEXT,
    webhook_event_id TEXT UNIQUE,
    device_id TEXT,
    sender TEXT,
    recipient TEXT,
    sim_number INTEGER,
    received_at TIMESTAMPTZ,
    raw_payload JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX messages_received_at_desc_idx ON messages (received_at DESC NULLS LAST, created_at DESC);
CREATE INDEX messages_direction_status_idx ON messages (direction, status);
CREATE INDEX messages_message_id_idx ON messages (message_id);
CREATE INDEX messages_device_id_idx ON messages (device_id);

CREATE TABLE webhook_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    event_id TEXT NOT NULL UNIQUE,
    event TEXT NOT NULL,
    device_id TEXT,
    webhook_id TEXT,
    payload JSONB NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    actor TEXT,
    action TEXT NOT NULL,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
