import type { SettingsResponse } from "./types";

export const API_BASE = import.meta.env.VITE_API_BASE_URL || "http://127.0.0.1:8080/api";

export function wsUrl(token: string) {
  const url = new URL(API_BASE.replace(/^http/, "ws") + "/ws");
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
  messages: (token: string) => request<import("./types").MessageRecord[]>("/messages?limit=250", token),
  settings: (token: string) => request<SettingsResponse>("/settings", token),
  saveSettings: (token: string, payload: Record<string, string>) =>
    request<{ message: string }>("/settings", token, { method: "POST", body: JSON.stringify(payload) }),
  registerWebhook: (token: string) =>
    request<{ message: string; webhook_url: string }>("/webhooks/smsgate/register", token, { method: "POST" }),
  sendMessage: (token: string, phone_number: string, message_content: string) =>
    request<{ message: string }>("/messages/send", token, {
      method: "POST",
      body: JSON.stringify({ phone_number, message_content })
    }),
  importInbox: (token: string, since: string, until: string) =>
    request<{ message: string }>("/messages/import-inbox", token, {
      method: "POST",
      body: JSON.stringify({ since, until })
    })
};
