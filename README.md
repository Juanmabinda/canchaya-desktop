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

## Code signing

Los binarios de Windows distribuidos en los [GitHub Releases](https://github.com/Juanmabinda/canchaya-desktop/releases) son firmados gratuitamente por la [SignPath Foundation](https://signpath.org/) con un certificado de código emitido por SignPath CA, como parte del programa de soporte a proyectos open source.

Los binarios de macOS son firmados con un certificado Apple Developer ID Application y notarizados a través del servicio de notarización de Apple.

## Releasing

El workflow `.github/workflows/release.yml` se dispara con un tag `v*.*.*`:

```bash
# 1. Bumpeá la version en src-tauri/tauri.conf.json + package.json
# 2. Commit
git tag v0.1.1
git push origin v0.1.1
```

El workflow:
- Hace checkout de `canchaya-print-agent` y compila el binario Go con CGO en cada plataforma (Mac arm64 y Win x64).
- Buildea Tauri firmando con `TAURI_SIGNING_PRIVATE_KEY` (GitHub Secret).
- Publica .app/.dmg/.msi + `.sig` + `latest.json` a un Release **draft**.
- Cuando publicas el Release, los clientes con la app abierta detectan el update en su próximo boot y se actualizan solos via Tauri Updater.

### Backup de la signing key

La keypair Ed25519 vive en `~/.tauri/canchaya-desktop` (privada) y `~/.tauri/canchaya-desktop.pub`. **Si perdés la privada, perdés la capacidad de firmar updates** y todos los clientes existentes se quedan trabados sin poder actualizarse — habría que distribuir un .app nuevo manualmente con una nueva pubkey.

Backupealo en un password manager. Y lo importante: la pubkey en `src-tauri/tauri.conf.json` tiene que matchear la privada — si las cambiás juntas, los clientes nuevos andan pero los viejos no se pueden actualizar (firma no valida).

## Roadmap

- [x] **Fase 1** — Wrapper WebView + sidecar Go + auto-pair.
- [x] **Fase 2** — Auto-update (GitHub Actions + Tauri Updater + signed releases).
- [ ] **Fase 3** — Installer Windows firmado (NSIS code-signing cert ~$200/año o SignPath gratis para OSS) + DMG Mac notarizado (Apple Developer account ya lo tenemos para iOS).
- [ ] **Fase 4** — Distribución: link en `/admin/printers` para descargar el wrapper.

## Notas

- Sin internet la app no funciona (igual que abrir canchaya.ar en el browser).
- Cambios en la web se reflejan al instante: `Ctrl+R` o reabrir la app.
- La DB sigue en el VPS — esto es un thin client.
