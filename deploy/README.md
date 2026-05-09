# Production Deployment

This directory contains examples for deploying the Rust SMSGate app on an Ubuntu VPS.

- `nginx.conf.example`: reverse proxy and static frontend config.
- `smsgate-backend.service.example`: systemd service for the Axum backend.

The recommended production path is:

- App source: `/opt/rust-smsgate`
- Backend binary: `/opt/rust-smsgate/backend/target/release/smsgate-backend`
- Frontend dist: `/opt/rust-smsgate/frontend/dist`
- Public API: `https://alsiyniisms.ddns.net/api`
- WebSocket: `wss://alsiyniisms.ddns.net/api/ws`
