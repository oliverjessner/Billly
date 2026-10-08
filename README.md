# Billly 🧮

A fully automated local bot that handles your small business finances

billly with 3 l

## Development

Use Node.js 22.12+ and install the pinned dependencies:

```bash
npm ci
npm run dev
```

The browser preview runs at `http://localhost:1420`. Start the desktop app and its frontend dev server with `npm run tauri`. Use `npm run tauri:cli -- <command>` for other Tauri CLI commands.

The UI uses [`oj-designsystem`](https://github.com/oliverjessner/oj-designsystem), pinned to `0.1.0`. `npm run build` copies its complete distribution, fonts, icons and license notices to `dist/public/vendor/oj-designsystem`; the dev server serves the same package files directly. All design-system assets are local, with no runtime CDN or font requests. Billly-specific layout styles follow the shared stylesheet in `src/styles.css`.

## Build

Create the static frontend in `dist` with `npm run build`. Create the macOS DMG with:

```bash
npm run build:adhoc-dmg
```

## SQLite database location

The app stores the SQLite database in the OS app data directory:

- macOS: `~/Library/Application Support/Billly/billly.sqlite`
- Windows: `%APPDATA%\\Billly\\billly.sqlite`
- Linux: `~/.local/share/Billly/billly.sqlite`
