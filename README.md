# Rust SMSGate

Fast SMSGate dashboard built with Rust, Axum, PostgreSQL, WebSocket realtime updates, and a React/Vite PWA frontend.

## Requirements

- Rust stable toolchain (`rustup`, `cargo`)
- Node.js 22+
- Docker Desktop or a PostgreSQL 16 server

## Local Setup

```powershell
cd "C:\MyProjects\Sms Gate\rust-smsgate"
docker compose up -d postgres
Copy-Item .env.example .env
```

Edit `.env`, especially:

- `DATABASE_URL`
- `APP_SECRET`
- `SETTINGS_ENCRYPTION_KEY`
- `ADMIN_USERNAME`
- `ADMIN_PASSWORD_HASH`

Generate a bcrypt hash:

```powershell
cargo run --manifest-path backend/Cargo.toml --bin hash-password -- "your-password"
```

Run backend:

```powershell
cd backend
cargo sqlx migrate run
cargo run
```

Run frontend:

```powershell
cd frontend
npm install
npm run dev
```

Default URLs:

- Backend: `http://127.0.0.1:8080`
- Frontend: `http://127.0.0.1:5173`
- Webhook endpoint: `https://your-domain.com/api/webhooks/smsgate`

## Production Notes

Run Axum behind Nginx on `127.0.0.1:8080`, proxy HTTPS traffic to it, and enable WebSocket upgrade for `/api/ws`.

