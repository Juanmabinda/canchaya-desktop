# CanchaYa POS (Desktop)

Wrapper de escritorio de [canchaya.ar](https://canchaya.ar/admin) para clubes. Construido con [Tauri](https://tauri.app/) (Rust + WebView nativo).

Reemplaza el flujo "abrir Chrome + abrir agente de impresión por separado" por un único `.exe` / `.app` que:

- Abre directo en `https://canchaya.ar/admin`.
- Embebe el agente de impresión (no hace falta abrirlo aparte ni pairear).
- Se actualiza solo (auto-update vía GitHub Releases).

## Por qué Tauri y no Electron

- `.exe` final ~10 MB (vs ~150 MB de Electron).
- ~30 MB de RAM idle (vs ~200 MB).
- Auto-update integrado y firmado con Ed25519.
- Arranca instantáneo en PCs de gama media (las que suelen tener los clubes).

## Requisitos para desarrollar

- [Rust stable](https://rustup.rs/) (`rustc 1.90+`)
- [Node.js 20+](https://nodejs.org/) (probado con 24)
- macOS / Windows 10+ / Linux

```bash
git clone <repo>
cd canchaya-desktop
npm install
npm run tauri dev    # arranca en modo desarrollo
```

## Build de producción

### macOS (Apple Silicon)

```bash
npm run tauri build
# → src-tauri/target/release/bundle/macos/CanchaYa POS.app
# → src-tauri/target/release/bundle/dmg/CanchaYa POS_<version>_aarch64.dmg
```

### Windows

```bash
npm run tauri build -- --target x86_64-pc-windows-msvc
# → src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/
```

### Solo `.app` (saltea DMG)

```bash
npm run tauri build -- --bundles app
```

## Estructura

```
canchaya-desktop/
├─ src/                       # Frontend mínimo (no se usa, solo placeholder)
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs              # Entry point (Windows-subsystem flag)
│  │  └─ lib.rs               # tauri::Builder — donde va a vivir el agente
│  ├─ tauri.conf.json         # config de la app (URL, ventana, bundle)
│  ├─ Cargo.toml              # deps Rust
│  └─ icons/                  # íconos generados desde el AppIcon-1024 de iOS
└─ package.json
```

## Configuración clave

`src-tauri/tauri.conf.json`:

| Campo | Valor |
|---|---|
| `productName` | `CanchaYa POS` |
| `identifier` | `app.canchalibre.desktop` |
| `app.windows[0].url` | `https://canchaya.ar/admin` |
| Tamaño ventana | 1280×800 (mínimo 900×600) |
| Decoraciones | `true` (modo kiosko viene después) |

Si no hay sesión, el server redirige al sign-in de canchaya.ar.

## Iconos

Generados con `npm run tauri icon <png-1024>` a partir del AppIcon que usa la app de iOS. Genera todas las variantes (PNG, `.icns`, `.ico`, mipmaps Android, AppIcon iOS).

## Roadmap

- [x] **Fase 1** — Wrapper WebView básico apuntando a canchaya.ar/admin.
- [ ] **Fase 2** — Integrar impresión directa (descubrir impresoras del sistema, ESC/POS, WebSocket al server). Sin agente separado.
- [ ] **Fase 3** — Auto-update (GitHub Releases + Tauri Updater).
- [ ] **Fase 4** — Installer Windows firmado (NSIS) + DMG Mac notarizado.

## Notas

- Sin internet la app no funciona (igual que abrir canchaya.ar en el browser).
- Cambios en la web se reflejan al instante: `Ctrl+R` o reabrir la app.
- La DB sigue en el VPS — esto es un thin client.
