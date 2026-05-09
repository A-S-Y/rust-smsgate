# Capacitor Android Notes

After the PWA is stable, Android packaging can be added from `frontend/`:

```powershell
npm install @capacitor/core @capacitor/cli @capacitor/android
npx cap init RustSMSGate com.alsiynii.smsgate --web-dir dist
npm run build
npx cap add android
npx cap sync android
npx cap open android
```

Set `VITE_API_BASE_URL=https://alsiyniisms.ddns.net/api` for production Android builds.
