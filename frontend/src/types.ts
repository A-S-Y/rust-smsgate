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
  has_password: boolean;
  has_webhook_signing_key: boolean;
}

export type RealtimeEvent =
  | { type: "message.created"; payload: MessageRecord }
  | { type: "message.updated"; payload: MessageRecord }
  | { type: "system.status"; payload: { status: string } };
