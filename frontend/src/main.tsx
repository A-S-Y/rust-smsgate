import React, { useEffect, useMemo, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { api, wsUrl } from "./api";
import type { MessageRecord, RealtimeEvent, SettingsResponse } from "./types";
import "./styles.css";

const tokenKey = "rust-smsgate-token";

interface Conversation {
  id: string;
  phone: string;
  title: string;
  messages: MessageRecord[];
  lastMessage: MessageRecord;
  receivedCount: number;
  sentCount: number;
}

function App() {
  const [token, setToken] = useState(() => localStorage.getItem(tokenKey));

  useEffect(() => {
    if ("serviceWorker" in navigator) {
      navigator.serviceWorker.register("/sw.js").catch(() => undefined);
    }
  }, []);

  if (!token) {
    return (
      <Login
        onLogin={(next) => {
          localStorage.setItem(tokenKey, next);
          setToken(next);
        }}
      />
    );
  }

  return (
    <Dashboard
      token={token}
      onLogout={() => {
        localStorage.removeItem(tokenKey);
        setToken(null);
      }}
    />
  );
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
        <label>
          اسم المستخدم
          <input value={username} onChange={(e) => setUsername(e.target.value)} />
        </label>
        <label>
          كلمة المرور
          <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} />
        </label>
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

  const stats = useMemo(
    () => ({
      total: messages.length,
      received: messages.filter((m) => m.direction === "received").length,
      sent: messages.filter((m) => m.direction === "sent").length,
    }),
    [messages]
  );

  async function reloadMessages() {
    const data = await api.messages(token);
    setMessages(sortMessages(data));
  }

  function upsertMessage(message: MessageRecord) {
    setMessages((prev) => sortMessages([message, ...prev.filter((item) => item.id !== message.id)]));
  }

  useEffect(() => {
    reloadMessages().catch((err) => setNotice(err.message));
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
          setMessages((prev) => sortMessages([realtime.payload, ...prev.filter((m) => m.id !== realtime.payload.id)]));
        }
        if (realtime.type === "message.updated") {
          setMessages((prev) => {
            const exists = prev.some((m) => m.id === realtime.payload.id);
            return sortMessages(exists ? prev.map((m) => (m.id === realtime.payload.id ? realtime.payload : m)) : [realtime.payload, ...prev]);
          });
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
          <span className={`connection ${connection}`}>
            {connection === "online" ? "متصل لحظياً" : connection === "connecting" ? "جاري الاتصال" : "غير متصل"}
          </span>
        </div>
        <nav>
          <button className={view === "messages" ? "active" : ""} onClick={() => setView("messages")}>
            المحادثات
          </button>
          <button className={view === "send" ? "active" : ""} onClick={() => setView("send")}>
            إرسال
          </button>
          <button className={view === "settings" ? "active" : ""} onClick={() => setView("settings")}>
            الإعدادات
          </button>
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
        {view === "messages" && <Conversations token={token} messages={messages} setNotice={setNotice} onMessageSaved={upsertMessage} />}
        {view === "send" && <Send token={token} setNotice={setNotice} onMessageSaved={upsertMessage} />}
        {view === "settings" && <Settings token={token} setNotice={setNotice} onSaved={reloadMessages} />}
      </main>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: number }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function Conversations({
  token,
  messages,
  setNotice,
  onMessageSaved,
}: {
  token: string;
  messages: MessageRecord[];
  setNotice: (value: string | null) => void;
  onMessageSaved: (message: MessageRecord) => void;
}) {
  const conversations = useMemo(() => buildConversations(messages), [messages]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [query, setQuery] = useState("");

  useEffect(() => {
    if (!selectedId && conversations.length > 0) setSelectedId(conversations[0].id);
    if (selectedId && conversations.length > 0 && !conversations.some((conversation) => conversation.id === selectedId)) {
      setSelectedId(conversations[0].id);
    }
  }, [conversations, selectedId]);

  const filtered = conversations.filter((conversation) => {
    const text = `${conversation.title} ${conversation.phone} ${conversation.lastMessage.message_content}`.toLowerCase();
    return text.includes(query.trim().toLowerCase());
  });
  const selected = conversations.find((conversation) => conversation.id === selectedId) || filtered[0] || null;

  return (
    <section className="conversation-shell">
      <aside className="conversation-list">
        <div className="conversation-list-head">
          <div>
            <h1>المحادثات</h1>
            <p>{conversations.length} محادثة نشطة</p>
          </div>
          <span className="live-dot">مباشر</span>
        </div>
        <input
          className="conversation-search"
          placeholder="بحث بالرقم أو نص الرسالة"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
        <div className="conversation-items">
          {filtered.map((conversation) => (
            <button
              key={conversation.id}
              className={`conversation-item ${selected?.id === conversation.id ? "active" : ""}`}
              onClick={() => setSelectedId(conversation.id)}
            >
              <Avatar value={conversation.title} />
              <div className="conversation-summary">
                <div>
                  <b dir="ltr">{conversation.title}</b>
                  <time>{formatConversationTime(conversation.lastMessage)}</time>
                </div>
                <p>{conversation.lastMessage.direction === "sent" ? "أنت: " : ""}{conversation.lastMessage.message_content}</p>
                <span>{conversation.messages.length} رسالة</span>
              </div>
            </button>
          ))}
          {filtered.length === 0 && <div className="empty small">لا توجد محادثات مطابقة.</div>}
        </div>
      </aside>

      <ConversationThread token={token} conversation={selected} setNotice={setNotice} onMessageSaved={onMessageSaved} />
    </section>
  );
}

function ConversationThread({
  token,
  conversation,
  setNotice,
  onMessageSaved,
}: {
  token: string;
  conversation: Conversation | null;
  setNotice: (value: string | null) => void;
  onMessageSaved: (message: MessageRecord) => void;
}) {
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const bottomRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    requestAnimationFrame(() => {
      bottomRef.current?.scrollIntoView({ behavior: "auto", block: "end" });
    });
  }, [conversation?.id]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [conversation?.messages.length]);

  if (!conversation) {
    return <div className="thread empty-thread"><div className="empty">اختر محادثة لعرض الرسائل والرد عليها.</div></div>;
  }

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    const message = text.trim();
    if (!message || sending || !conversation) return;

    setSending(true);
    setNotice(null);
    try {
      const res = await api.sendMessage(token, conversation.phone, message);
      onMessageSaved(res.data);
      setNotice(res.message);
      setText("");
    } catch (err) {
      setNotice(err instanceof Error ? err.message : "فشل إرسال الرد");
    } finally {
      setSending(false);
    }
  }

  return (
    <div className="thread">
      <header className="thread-header">
        <Avatar value={conversation.title} />
        <div>
          <h2 dir="ltr">{conversation.title}</h2>
          <p>{conversation.receivedCount} وارد · {conversation.sentCount} صادر · {conversation.phone}</p>
        </div>
      </header>

      <div className="thread-body">
        {conversation.messages.map((message, index) => {
          const previous = conversation.messages[index - 1];
          const showDate = !previous || dateKey(previous) !== dateKey(message);
          return (
            <React.Fragment key={message.id}>
              {showDate && <div className="date-divider">{formatDateGroup(message)}</div>}
              <MessageBubble message={message} />
            </React.Fragment>
          );
        })}
        <div ref={bottomRef} />
      </div>

      <form className="reply-bar" onSubmit={submit}>
        <textarea
          placeholder={`اكتب رداً إلى ${conversation.title}`}
          value={text}
          onChange={(event) => setText(event.target.value)}
          rows={1}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              event.currentTarget.form?.requestSubmit();
            }
          }}
        />
        <button disabled={sending || !text.trim()}>{sending ? "..." : "إرسال"}</button>
      </form>
    </div>
  );
}

function MessageBubble({ message }: { message: MessageRecord }) {
  const outgoing = message.direction === "sent";
  return (
    <div className={`bubble-row ${outgoing ? "outgoing" : "incoming"}`}>
      <div className="bubble">
        <p>{message.message_content}</p>
        <footer><span>{message.status}</span><time>{formatTime(message)}</time></footer>
      </div>
    </div>
  );
}

function Avatar({ value }: { value: string }) {
  const label = value.replace(/[^\dA-Za-z\u0600-\u06FF]/g, "").slice(-2) || "؟";
  return <span className="avatar">{label}</span>;
}

function Send({
  token,
  setNotice,
  onMessageSaved,
}: {
  token: string;
  setNotice: (value: string | null) => void;
  onMessageSaved: (message: MessageRecord) => void;
}) {
  const [phone, setPhone] = useState("");
  const [text, setText] = useState("");

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    const res = await api.sendMessage(token, phone, text);
    onMessageSaved(res.data);
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

function Settings({
  token,
  setNotice,
  onSaved,
}: {
  token: string;
  setNotice: (value: string | null) => void;
  onSaved: () => Promise<void>;
}) {
  const [settings, setSettings] = useState<SettingsResponse | null>(null);
  const [form, setForm] = useState<Record<string, string>>({});
  const [range, setRange] = useState(() => ({
    since: new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString(),
    until: new Date().toISOString(),
  }));

  useEffect(() => {
    api.settings(token).then((data) => {
      setSettings(data);
      setForm({
        server_url: data.server_url || "https://api.sms-gate.app",
        username: data.username || "",
        device_id: data.device_id || "",
        webhook_public_url: data.webhook_public_url || "",
        messages_retention_days: String(data.messages_retention_days || 30),
      });
    });
  }, [token]);

  async function save(event: React.FormEvent) {
    event.preventDefault();
    const res = await api.saveSettings(token, form);
    setNotice(res.message);
    await onSaved();
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
        {["server_url", "username", "password", "device_id", "webhook_public_url", "webhook_signing_key", "messages_retention_days"].map((key) => (
          <label key={key}>{labelFor(key)}
            <input
              dir="ltr"
              type={key.includes("password") || key.includes("key") ? "password" : key === "messages_retention_days" ? "number" : "text"}
              min={key === "messages_retention_days" ? 1 : undefined}
              max={key === "messages_retention_days" ? 3650 : undefined}
              value={form[key] || ""}
              onChange={(e) => setForm((prev) => ({ ...prev, [key]: e.target.value }))}
            />
          </label>
        ))}
        <button>حفظ الإعدادات</button>
      </form>
      <div className="actions"><button onClick={registerWebhook}>تسجيل Webhook</button></div>
      <div className="import-box">
        <h2>استيراد الرسائل السابقة</h2>
        <input dir="ltr" value={range.since} onChange={(e) => setRange((prev) => ({ ...prev, since: e.target.value }))} />
        <input dir="ltr" value={range.until} onChange={(e) => setRange((prev) => ({ ...prev, until: e.target.value }))} />
        <button onClick={importInbox}>طلب الاستيراد</button>
      </div>
    </section>
  );
}

function buildConversations(messages: MessageRecord[]): Conversation[] {
  const groups = new Map<string, MessageRecord[]>();
  for (const message of messages) {
    const phone = normalizePhone(counterpart(message));
    if (!groups.has(phone)) groups.set(phone, []);
    groups.get(phone)!.push(message);
  }

  return Array.from(groups.entries())
    .map(([phone, group]) => {
      const sorted = [...group].sort((a, b) => timestamp(a) - timestamp(b));
      const lastMessage = sorted[sorted.length - 1];
      return {
        id: phone,
        phone,
        title: phone,
        messages: sorted,
        lastMessage,
        receivedCount: sorted.filter((message) => message.direction === "received").length,
        sentCount: sorted.filter((message) => message.direction === "sent").length,
      };
    })
    .sort((a, b) => timestamp(b.lastMessage) - timestamp(a.lastMessage));
}

function counterpart(message: MessageRecord) {
  return message.direction === "received" ? message.sender || message.phone_number : message.recipient || message.phone_number;
}

function normalizePhone(value: string) {
  return value.trim() || "Unknown";
}

function timestamp(message: MessageRecord) {
  return new Date(message.received_at || message.created_at).getTime();
}

function sortMessages(messages: MessageRecord[]) {
  return [...messages].sort((a, b) => timestamp(b) - timestamp(a));
}

function formatTime(message: MessageRecord) {
  return new Date(message.received_at || message.created_at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function formatConversationTime(message: MessageRecord) {
  const date = new Date(message.received_at || message.created_at);
  const now = new Date();
  if (date.toDateString() === now.toDateString()) return formatTime(message);
  return date.toLocaleDateString();
}

function dateKey(message: MessageRecord) {
  return new Date(message.received_at || message.created_at).toDateString();
}

function formatDateGroup(message: MessageRecord) {
  return new Date(message.received_at || message.created_at).toLocaleDateString(undefined, {
    weekday: "long",
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

function labelFor(key: string) {
  return ({
    server_url: "رابط SMS Gate API",
    username: "اسم المستخدم",
    password: "كلمة المرور",
    device_id: "معرف الجهاز",
    webhook_public_url: "رابط HTTPS العام",
    webhook_signing_key: "Webhook Signing Key",
    messages_retention_days: "عرض رسائل آخر عدد أيام",
  } as Record<string, string>)[key];
}

createRoot(document.getElementById("root")!).render(<App />);
