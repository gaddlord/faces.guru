# faces.guru

A private, **local-only** AI app for adult/NSFW image generation, face-swap, and
image→video — phone/tablet client, everything else running on your own Mac. See
[ROADMAP.md](ROADMAP.md) for the full product plan and rationale; this README is the
build/run guide for what's implemented.

> **Shape:** pure-JS + Capacitor client (APK/IPA) → talks over a **Tailscale** mesh to a
> **Rust/Axum** backend on the Mac. The backend owns a SQLite job queue and a worker that
> drives local model servers (ComfyUI, LM Studio, InSwapper, LTX-Video). No cloud, no auth,
> no payments. See ROADMAP §0.3 for the (unchanged) legal responsibilities of running this.

## What's implemented

All four core features plus prompt-assist, history/gallery, and settings are wired
end-to-end, with a **mock mode** (default) so the whole app runs and is testable without
any model servers installed:

| Feature | Status | Path |
|---|---|---|
| Prompt → image (+ negative/context conditioning) | ✅ | `image` job → ComfyUI SDXL graph |
| AI prompt enhancer (prose/tags) | ✅ | `POST /api/prompt/enhance` → LM Studio (raw-prompt fallback) |
| Face swap | ✅ | `swap` job → InSwapper microservice |
| Image → video | ✅ | `video` job → LTX-Video/Wan microservice |
| Job queue + status polling | ✅ | SQLite `jobs` table + serial worker |
| History / gallery | ✅ | `GET /api/jobs` + media serving |
| Settings (backend address) | ✅ | client, persisted to localStorage |

In **mock mode** the worker fabricates real, viewable placeholder media (gradient PNGs for
image/swap, a GIF for video) so you can exercise the entire pipeline. Flip `FG_MOCK=false`
on the Mac to use the real models.

## Repo layout

```
backend/   Rust/Axum API + queue + worker + model-server clients
client/    Pure-JS (Vite) app + Capacitor config for APK/IPA
ROADMAP.md Full product plan & trackable roadmap
```

## Backend

Requires Rust (stable). From `backend/`:

```bash
cp .env.example .env        # optional; all values have defaults
cargo run                   # starts on FG_BIND (default 0.0.0.0:8080), mock mode on
```

Config (env or `.env`, see `.env.example`): `FG_BIND`, `FG_DB`, `FG_MEDIA_DIR`,
`FG_MOCK`, `FG_PROMPT_MODE` (`prose`|`tags`), `FG_COMFYUI_URL`, `FG_IMAGE_CKPT`,
`FG_LMSTUDIO_URL`, `FG_LMSTUDIO_MODEL`, `FG_FACESWAP_URL`, `FG_VIDEO_URL`.

### HTTP API

| Method & path | Purpose |
|---|---|
| `GET /health` | liveness |
| `POST /api/prompt/enhance` | `{idea, context?, negative?}` → `{positive, negative, enhanced}` |
| `POST /api/jobs` | `{type:"image"\|"swap"\|"video", params, input_media_ids?}` → job |
| `GET /api/jobs/:id` | poll one job (status, progress, output_media_ids) |
| `GET /api/jobs` | recent history (newest first) |
| `POST /api/media` | multipart `file` upload → `{id}` |
| `GET /api/media/:id` | serve a stored media file |

Job `params` by type:
- **image**: `positive`, `negative`, `width`, `height`, `steps`, `cfg`, `seed`, `sampler` (default `dpmpp_2m`), `scheduler` (default `karras`), `vae` (optional VAE filename override)
- **swap**: `restore` (bool); `input_media_ids = [source_face, target_image]`
- **video**: `prompt`, `fps` (8–30), `duration` (1–6s); `input_media_ids = [still]`

### Model-server contracts (used when `FG_MOCK=false`)

- **ComfyUI** (`FG_COMFYUI_URL`, default `:8188`): standard ComfyUI HTTP API. The worker
  submits an SDXL text2img graph to `POST /prompt`, polls `GET /history/{id}`, and downloads
  the result from `GET /view`. Set `FG_IMAGE_CKPT` to a checkpoint present in ComfyUI.
- **LM Studio** (`FG_LMSTUDIO_URL`, default `:1234`): OpenAI-compatible
  `POST /v1/chat/completions`. The enhancer asks for `{"positive","negative"}` JSON; if the
  server is down, generation still works using the raw prompt.
- **Face-swap** (`FG_FACESWAP_URL`, default `:5000`): `POST /swap` multipart
  `source`, `target`, `restore` → image bytes. A minimal reference implementation is in
  [`backend/services/faceswap_service.py`](backend/services/faceswap_service.py).
- **Video** (`FG_VIDEO_URL`, default `:5001`): `POST /generate` multipart
  `image`, `prompt`, `fps`, `duration` → mp4 bytes. Reference stub in
  [`backend/services/video_service.py`](backend/services/video_service.py).

## Client

Requires Node. From `client/`:

```bash
npm install
npm run dev        # http://localhost:5173 (talks to backend at 127.0.0.1:8089 by default)
npm run build      # production bundle in dist/
```

The backend address is set in the **Settings** tab and saved to localStorage. On a device,
set it to the Mac's Tailscale IP + port (e.g. `http://100.x.y.z:8080`).

### Capacitor (APK / IPA)

```bash
npm run build
npx cap add android      # then: npx cap open android  → build signed APK
npx cap add ios          # then: npx cap open ios       → run via Xcode/TestFlight
npx cap sync
```

`capacitor.config.json` sets `androidScheme: http` + `cleartext: true` so the webview can
reach the Mac's `http://` backend over the (already-encrypted) Tailscale mesh.

## Mac deployment notes (ROADMAP Phase 6)

- The Mac is the whole backend — keep it awake: `caffeinate -dimsu cargo run` (or a launchd
  job). FileVault on; back up the media dir.
- Per-job safety: video duration/fps are capped (1–6s, 8–30fps); model calls have timeouts.
- Tailscale must be running on Mac + device; bind the backend to the Tailscale interface or
  `0.0.0.0`.

## Legal

This tool intentionally has **no** content-moderation pipeline (per the product direction in
ROADMAP §0.2). That does not remove the law: CSAM and non-consensual intimate deepfakes of
real people are criminal regardless. Keep generations synthetic/consented — see ROADMAP §0.3.
