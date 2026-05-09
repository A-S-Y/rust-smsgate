import type { MessageRecord, SettingsResponse, SyncLogResponse } from "./types";

export const API_BASE = import.meta.env.VITE_API_BASE_URL || "/api";

export function wsUrl(token: string) {
  const apiUrl = new URL(API_BASE, window.location.origin);
  apiUrl.protocol = apiUrl.protocol === "https:" ? "wss:" : "ws:";
  apiUrl.pathname = `${apiUrl.pathname.replace(/\/$/, "")}/ws`;
  const url = apiUrl;
  url.searchParams.set("token", token);
  return url.toString();
}

async function request<T>(path: string, token: string | null, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set("Content-Type", "application/json");
  if (token) headers.set("Authorization", `Bearer ${token}`);

  const res = await fetch(`${API_BASE}${path}`, { ...init, headers });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) {
    throw new Error(data.error || `Request failed with status ${res.status}`);
  }
  return data as T;
}

export const api = {
  login: (username: string, password: string) =>
    request<{ token: string }>("/auth/login", null, {
      method: "POST",
      body: JSON.stringify({ username, password })
    }),
  messages: (token: string) => request<MessageRecord[]>("/messages?limit=500", token),
  settings: (token: string) => request<SettingsResponse>("/settings", token),
  syncLog: (token: string) => request<SyncLogResponse>("/diagnostics/sync-log", token),
  saveSettings: (token: string, payload: Record<string, string>) =>
    request<{ message: string }>("/settings", token, {
      method: "POST",
      body: JSON.stringify({
        ...payload,
        messages_retention_days: Number(payload.messages_retention_days || 30)
      })
    }),
  registerWebhook: (token: string) =>
    request<{ message: string; webhook_url: string }>("/webhooks/smsgate/register", token, { method: "POST" }),
  sendMessage: (token: string, phone_number: string, message_content: string) =>
    request<{ message: string; data: MessageRecord }>("/messages/send", token, {
      method: "POST",
      body: JSON.stringify({ phone_number, message_content })
    }),
  importInbox: (token: string, since: string, until: string) =>
    request<{ message: string }>("/messages/import-inbox", token, {
      method: "POST",
      body: JSON.stringify({ since, until })
    })
};
