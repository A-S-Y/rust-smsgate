export interface MessageRecord {
  id: string;
  direction: "sent" | "received";
  status: string;
  phone_number: string;
  message_content: string;
  message_id?: string | null;
  webhook_event_id?: string | null;
  device_id?: string | null;
  sender?: string | null;
  recipient?: string | null;
  sim_number?: number | null;
  received_at?: string | null;
  created_at: string;
  updated_at: string;
}

export interface SettingsResponse {
  server_url?: string | null;
  username?: string | null;
  device_id?: string | null;
  webhook_public_url?: string | null;
  messages_retention_days: number;
  has_password: boolean;
  has_webhook_signing_key: boolean;
}

export interface SyncLogResponse {
  summary: {
    sent_messages: number;
    received_messages: number;
    recent_webhooks: number;
    recent_audit_logs: number;
  };
  recent_webhooks: Array<{
    event_id: string;
    event: string;
    device_id?: string | null;
    webhook_id?: string | null;
    received_at: string;
    payload: unknown;
  }>;
  recent_audit_logs: Array<{
    actor?: string | null;
    action: string;
    metadata?: unknown;
    created_at: string;
  }>;
  recent_outgoing_messages: Array<{
    id: string;
    status: string;
    phone_number: string;
    message_id?: string | null;
    webhook_event_id?: string | null;
    created_at: string;
    updated_at: string;
    raw_payload?: unknown;
  }>;
}

export type RealtimeEvent =
  | { type: "message.created"; payload: MessageRecord }
  | { type: "message.updated"; payload: MessageRecord }
  | { type: "system.status"; payload: { status: string } };
