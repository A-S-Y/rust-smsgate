import React, { useEffect, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import { api, wsUrl } from "./api";
import type { MessageRecord, RealtimeEvent, SettingsResponse } from "./types";
import "./styles.css";

const tokenKey = "rust-smsgate-token";

function App() {
  const [token, setToken] = useState(() => localStorage.getItem(tokenKey));

  useEffect(() => {
    if ("serviceWorker" in navigator) {
      navigator.serviceWorker.register("/sw.js").catch(() => undefined);
    }
  }, []);

  if (!token) return <Login onLogin={(next) => {
    localStorage.setItem(tokenKey, next);
    setToken(next);
  }} />;

  return <Dashboard token={token} onLogout={() => {
    localStorage.removeItem(tokenKey);
    setToken(null);
  }} />;
}

function Login({ onLogin }: { onLogin: (token: string) => void }) {
  const [username, setUsername] = useState("admin");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    setError(null);
    try {
      const res = await api.login(username, password);
      onLogin(res.token);
    } catch (err) {
      setError(err instanceof Error ? err.message : "فشل تسجيل الدخول");
    }
  }

  return (
    <main className="login-shell">
      <form className="login-card" onSubmit={submit}>
        <div className="brand-mark">SMS</div>
        <h1>Rust SMSGate</h1>
        <p>لوحة رسائل لحظية مبنية بـ Axum وPostgreSQL.</p>
        {error && <div className="alert danger">{error}</div>}
        <label>اسم المستخدم<input value={username} onChange={(e) => setUsername(e.target.value)} /></label>
        <label>كلمة المرور<input type="password" value={password} onChange={(e) => setPassword(e.target.value)} /></label>
        <button>دخول</button>
      </form>
    </main>
  );
}

function Dashboard({ token, onLogout }: { token: string; onLogout: () => void }) {
  const [view, setView] = useState<"messages" | "send" | "settings">("messages");
  const [messages, setMessages] = useState<MessageRecord[]>([]);
  const [connection, setConnection] = useState<"connecting" | "online" | "offline">("connecting");
  const [notice, setNotice] = useState<string | null>(null);

  const stats = useMemo(() => ({
    total: messages.length,
    received: messages.filter((m) => m.direction === "received").length,
    sent: messages.filter((m) => m.direction === "sent").length
  }), [messages]);

  useEffect(() => {
    api.messages(token).then(setMessages).catch((err) => setNotice(err.message));
  }, [token]);

  useEffect(() => {
    let closed = false;
    let socket: WebSocket | null = null;
    let retry: number | undefined;

    const connect = () => {
      setConnection("connecting");
      socket = new WebSocket(wsUrl(token));
      socket.onopen = () => setConnection("online");
      socket.onclose = () => {
        setConnection("offline");
        if (!closed) retry = window.setTimeout(connect, 1500);
      };
      socket.onerror = () => setConnection("offline");
      socket.onmessage = (event) => {
        const realtime = JSON.parse(event.data) as RealtimeEvent;
        if (realtime.type === "message.created") {
          setMessages((prev) => [realtime.payload, ...prev.filter((m) => m.id !== realtime.payload.id)]);
        }
        if (realtime.type === "message.updated") {
          setMessages((prev) => prev.map((m) => (m.id === realtime.payload.id ? realtime.payload : m)));
        }
      };
    };

    connect();
    return () => {
      closed = true;
      if (retry) window.clearTimeout(retry);
      socket?.close();
    };
  }, [token]);

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div>
          <div className="brand-mark small">SMS</div>
          <h2>بوابة الرسائل</h2>
          <span className={`connection ${connection}`}>{connection === "online" ? "متصل لحظياً" : connection === "connecting" ? "جاري الاتصال" : "غير متصل"}</span>
        </div>
        <nav>
          <button className={view === "messages" ? "active" : ""} onClick={() => setView("messages")}>الرسائل</button>
          <button className={view === "send" ? "active" : ""} onClick={() => setView("send")}>إرسال</button>
          <button className={view === "settings" ? "active" : ""} onClick={() => setView("settings")}>الإعدادات</button>
          <button onClick={onLogout}>خروج</button>
        </nav>
      </aside>

      <main className="content">
        {notice && <div className="alert warning">{notice}</div>}
        <section className="metric-grid">
          <Metric label="الإجمالي" value={stats.total} />
          <Metric label="الوارد" value={stats.received} />
          <Metric label="الصادر" value={stats.sent} />
        </section>
        {view === "messages" && <Messages messages={messages} />}
        {view === "send" && <Send token={token} setNotice={setNotice} />}
        {view === "settings" && <Settings token={token} setNotice={setNotice} />}
      </main>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: number }) {
  return <div className="metric"><span>{label}</span><strong>{value}</strong></div>;
}

function Messages({ messages }: { messages: MessageRecord[] }) {
  return (
    <section className="panel">
      <div className="panel-head"><h1>الرسائل اللحظية</h1><p>تظهر الرسائل فور وصول Webhook بدون polling.</p></div>
      <div className="message-list">
        {messages.map((message) => (
          <article key={message.id} className={`message-row ${message.direction}`}>
            <div>
              <span className="pill">{message.direction === "received" ? "وارد" : "صادر"}</span>
              <b dir="ltr">{message.direction === "received" ? message.sender || message.phone_number : message.recipient || message.phone_number}</b>
            </div>
            <p>{message.message_content}</p>
            <footer><span>{message.status}</span><span>{new Date(message.received_at || message.created_at).toLocaleString()}</span></footer>
          </article>
        ))}
        {messages.length === 0 && <div className="empty">لا توجد رسائل بعد.</div>}
      </div>
    </section>
  );
}

function Send({ token, setNotice }: { token: string; setNotice: (value: string | null) => void }) {
  const [phone, setPhone] = useState("");
  const [text, setText] = useState("");

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    const res = await api.sendMessage(token, phone, text);
    setNotice(res.message);
    setPhone("");
    setText("");
  }

  return (
    <section className="panel narrow">
      <h1>إرسال رسالة</h1>
      <form className="form" onSubmit={submit}>
        <label>رقم الهاتف<input dir="ltr" value={phone} onChange={(e) => setPhone(e.target.value)} placeholder="+966500000000" /></label>
        <label>نص الرسالة<textarea value={text} onChange={(e) => setText(e.target.value)} rows={6} /></label>
        <button>إرسال</button>
      </form>
    </section>
  );
}

function Settings({ token, setNotice }: { token: string; setNotice: (value: string | null) => void }) {
  const [settings, setSettings] = useState<SettingsResponse | null>(null);
  const [form, setForm] = useState<Record<string, string>>({});
  const [range, setRange] = useState(() => ({
    since: new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString(),
    until: new Date().toISOString()
  }));

  useEffect(() => {
    api.settings(token).then((data) => {
      setSettings(data);
      setForm({
        server_url: data.server_url || "https://api.sms-gate.app",
        username: data.username || "",
        device_id: data.device_id || "",
        webhook_public_url: data.webhook_public_url || ""
      });
    });
  }, [token]);

  async function save(event: React.FormEvent) {
    event.preventDefault();
    const res = await api.saveSettings(token, form);
    setNotice(res.message);
  }

  async function registerWebhook() {
    const res = await api.registerWebhook(token);
    setNotice(`${res.message}: ${res.webhook_url}`);
  }

  async function importInbox() {
    const res = await api.importInbox(token, range.since, range.until);
    setNotice(res.message);
  }

  return (
    <section className="panel narrow">
      <h1>الإعدادات</h1>
      {settings && <p>كلمة مرور SMS Gate: {settings.has_password ? "مخزنة" : "غير مخزنة"} | Signing Key: {settings.has_webhook_signing_key ? "مخزن" : "غير مخزن"}</p>}
      <form className="form" onSubmit={save}>
        {["server_url", "username", "password", "device_id", "webhook_public_url", "webhook_signing_key"].map((key) => (
          <label key={key}>{labelFor(key)}
            <input
              dir="ltr"
              type={key.includes("password") || key.includes("key") ? "password" : "text"}
              value={form[key] || ""}
              onChange={(e) => setForm((prev) => ({ ...prev, [key]: e.target.value }))}
            />
          </label>
        ))}
        <button>حفظ الإعدادات</button>
      </form>
      <div className="actions">
        <button onClick={registerWebhook}>تسجيل Webhook</button>
      </div>
      <div className="import-box">
        <h2>استيراد الرسائل السابقة</h2>
        <input dir="ltr" value={range.since} onChange={(e) => setRange((prev) => ({ ...prev, since: e.target.value }))} />
        <input dir="ltr" value={range.until} onChange={(e) => setRange((prev) => ({ ...prev, until: e.target.value }))} />
        <button onClick={importInbox}>طلب الاستيراد</button>
      </div>
    </section>
  );
}

function labelFor(key: string) {
  return ({
    server_url: "رابط SMS Gate API",
    username: "اسم المستخدم",
    password: "كلمة المرور",
    device_id: "معرف الجهاز",
    webhook_public_url: "رابط HTTPS العام",
    webhook_signing_key: "Webhook Signing Key"
  } as Record<string, string>)[key];
}

createRoot(document.getElementById("root")!).render(<App />);
