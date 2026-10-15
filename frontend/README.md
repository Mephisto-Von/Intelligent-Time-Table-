# TimetableMaster — frontend

React 19 + TypeScript + Vite single-page app for the TimetableMaster API (see the [root README](../README.md)).

```bash
npm install
npm run dev      # Vite dev server on :5173, proxies /api to http://localhost:8080
npm run build    # type-check + production build into dist/ (served by the Rust backend)
npm run preview  # preview the production build
```

Layout:

- `src/api.ts` — typed fetch client and every API model.
- `src/auth.tsx` — session context (JWT in `localStorage`, `/api/auth/me` hydration).
- `src/components/Layout.tsx` — sidebar/topbar shell; `TimetableGrid.tsx` — drag & drop grid.
- `src/pages/*` — dashboard, timetable, school setup, leave, cover, calendar, settings.
- `src/ui.tsx` — buttons, cards, modals, toasts, formatting helpers.
