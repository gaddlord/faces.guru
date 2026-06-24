# NSFW AI Image/Video App
## Complete Build Plan & Trackable Roadmap

> **Product in one line:** A private (non-distributed) mobile app (phone + tablet) that lets you generate adult/NSFW images from AI prompts, refine them with extra context, face-swap, and animate stills into short videos. Thin **pure-JavaScript** client wrapped into APK/IPA with **Capacitor**; the **entire backend + all models run locally on a MacBook M5 (128GB)**, reached from the phone over a **Tailscale** mesh. **No cloud GPU, no payments, no public distribution** (AWS optional, storage-only). Target experience = "Grok image gen × FaceAI face-swap."

---

## 0. READ THIS FIRST — Hard Constraints That Shape Everything

These are not optional opinions; they will block or kill the project if ignored. The architecture below is designed around them.

### 0.1 No public distribution → no app-store problem (but you still self-sign)
- You're **not publishing** to the App Store or Google Play, so their NSFW bans are irrelevant. Good — that removes the biggest distribution headache.
- You still need to get the app onto devices:
  - **Android**: **Capacitor builds a signed APK** you install directly (sideload). Trivial — enable "install unknown apps" and copy the APK over.
  - **iOS**: no public store means you install via **Xcode to your own device(s)**, or **TestFlight** for a small private group, or an **Apple Developer Enterprise** account for internal distribution. A free Apple ID signs an IPA that lasts 7 days; a paid Developer account ($99/yr) gives 1-year provisioning. This is the only real friction on iOS — plan for it.

### 0.2 Payments / auth / moderation infra: removed
- **No payments, no auth, no caches, no CSAM-scanning pipeline, no ToS** — dropped per your direction. This is a single-operator private tool, so the heavy compliance/account machinery is gone. The infra in §2 is scaled down accordingly.

### 0.3 One honest legal caveat (not infra — just don't skip reading it)
Removing the *tooling* doesn't remove the *law*. Two things remain criminal regardless of how private the tool is: **(a) any sexual depiction of minors (CSAM)** and **(b) non-consensual intimate deepfakes of real, identifiable people** (US TAKE IT DOWN Act 2025, UK OSA, EU, many states). The plan no longer builds detection for these because you asked to drop it — that places the responsibility entirely on you as the operator. Keep generations synthetic/consented and you're fine.

- **Model-provider policy check**: Grok/xAI and most hosted face-swap APIs **prohibit** NSFW in their terms, so you can't route NSFW through them. Use **self-hosted open models**: SDXL/Flux + LoRAs (image), InsightFace/InSwapper (face), an open img2vid model (video). "Grok-like" = *UX inspiration*, not the Grok API.

> **Confirmed shape (revised):** pure-JS + Capacitor → APK/IPA · **everything runs on the MacBook M5 (128GB)** — prompt-assist LLM + image gen + face-swap + video · phone reaches the Mac over a **Tailscale** mesh (works on any network) · **AWS is optional** (only if you want S3+CloudFront media storage/CDN or a public domain) · no auth, no caches, no payments, no CSAM/ToS.
>
> **What this buys you:** ~$0 ongoing GPU cost (you own the hardware; just electricity). **What it costs you:** the Mac is a single point of failure and must be on/awake, and video gen is slow on Apple Silicon (see §6 caveats).

---

## 1. Product Scope

### 1.1 Core features (the four you asked for)
1. **Prompt → NSFW image** — user writes/AI-assists a prompt; backend generates image(s).
2. **Additional context prompt** — secondary prompt / negative prompt / style & reference conditioning to refine output.
3. **Face swap** — take a face from image A, apply to generated/uploaded image B (consent-gated).
4. **Image + prompt → video** — animate a still into a short clip (img2vid / motion).

### 1.2 Supporting features
- **Prompt builder / enhancer** (LLM expands a short idea into a strong diffusion prompt — the "Grok-like" assist; see §6 for model choice).
- Generation **queue + history/gallery**.
- Settings.

### 1.3 Explicitly out of scope
- Auth / accounts, payments, content-moderation pipeline, caches — all removed.
- Social feed / public sharing, real-time video, multi-minute video, voice, native store apps.

---

## 2. Architecture

> **Everything runs on the MacBook M5.** All three model roles live on one machine: (1) **Prompt-assist LLM** (text→text — writes a better prompt, can't make pixels), (2) **Image/diffusion models** (Flux/SDXL/Pony — generate images + face-swap), (3) **Video model** (LTX-Video / Wan img2vid — still→clip). They run **serially per job** and load on demand, so 128GB is plenty. The phone reaches the Mac over Tailscale.

### 2.1 High-level
```
┌─────────────────────────────────────────────────────────────────┐
│  Pure-JS client wrapped by Capacitor → APK (Android) / IPA (iOS)│
│  - Thin: prompt UI, upload, poll job status, view media         │
│  - Tailscale client built in → reaches the Mac on any network   │
└───────────────┬─────────────────────────────────────────────────┘
                │ HTTPS over Tailscale mesh (E2E, no auth needed)
╔═══════════════▼═════════════════════════════════════════════════╗
║  MacBook M5 (128GB) — YOUR machine, home network                ║
║                                                                 ║
║  ┌───────────────────────────────────────────────────────────┐  ║
║  │ Rust API (Axum): job submit / status / serve media        │  ║
║  └───────────────┬───────────────────────────────────────────┘  ║
║                  │  local job queue (Postgres/SQLite table)     ║
║  ┌───────────────▼────────────────────────────────────────────┐ ║
║  │ Rust worker → loads models on demand (serial per job):     │ ║
║  │  • Prompt-assist LLM  (LM Studio/MLX, ~24B abliterated)    │ ║
║  │  • Image gen          (Flux / SDXL / Pony via ComfyUI-MPS) │ ║
║  │  • Face-swap          (InSwapper via CoreML/ONNX)          │ ║
║  │  • Video (img2vid)    (LTX-Video / Wan, MPS) — slow        │ ║
║  └───────────────┬────────────────────────────────────────────┘ ║
║          ┌───────▼────────┐                                     ║
║          │ local media dir │  (or S3 if you opt into AWS below) ║
║          └────────────────┘                                     ║
╚═════════════════════════════════════════════════════════════════╝
        (OPTIONAL) AWS: S3 + CloudFront only, if you want media
        stored/served off-device or a public domain. Not required.
```

### 2.2 Why this shape (single Mac host)
- **JS client = thin**: never runs models, only renders UI, uploads inputs, polls job state, shows media. **Capacitor** wraps the web bundle into native APK/IPA and exposes native bits (file picker, camera) via plugins. It also runs a **Tailscale** client so the phone reaches the Mac from anywhere (home Wi-Fi, cellular, traveling).
- **Everything else is on the Mac.** A single **Rust API (Axum + Tokio)** accepts jobs, a local queue feeds a **Rust worker**, and the worker drives the models. No cloud GPU, no SQS, no RDS — just one machine.
- **Models load on demand, one job at a time.** A job is image, swap, or video. The worker keeps the small prompt-assist LLM warm and loads the heavy diffusion/video model for the duration of the job, then frees it. Serial execution means 128GB is never overcommitted.
  - **Image / face-swap**: fast enough on the M5 (seconds to low-tens-of-seconds).
  - **Video**: slow on Apple Silicon (minutes). Acceptable for a personal tool where you wait; offload to a rented GPU later if needed.
- The worker shells out to local model servers — **LM Studio/MLX** for the LLM, **ComfyUI (MPS/MLX)** for diffusion, **ONNX/CoreML** for InSwapper — over localhost. Rust orchestrates.
- **Async job model**: submit → job id → poll/WSS → media URL. No synchronous HTTP for generation (jobs are slow, especially video).
- **AWS is optional and storage-only.** If you want generations backed up / served off the Mac, add S3 + CloudFront for `media`. Otherwise the Mac serves media directly over Tailscale and there is no cloud at all.

### 2.3 Tech choices
| Layer | Choice | Notes |
|---|---|---|
| Client | **Pure JavaScript** (vanilla or a light framework — Vite build) wrapped by **Capacitor** + Tailscale client | Single web codebase → APK + IPA; responsive phone + tablet; reaches the Mac on any network |
| Host | **MacBook M5, 128GB** | Runs the entire backend + all models |
| API | Rust + **Axum**, Tokio, SQLx — on the Mac | Async, typed, small |
| Queue | **Local queue** — a `jobs` table polled by the worker (no SQS) | Single machine; no cloud queue needed |
| DB | **SQLite** (or local Postgres) | Just `jobs` + `media` metadata |
| Media store | **Local directory** on the Mac (optional: S3 + CloudFront) | Served over Tailscale; cloud only if you want off-device backup |
| Networking | **Tailscale mesh** (E2E) | Phone ↔ Mac from anywhere; no public ports |
| LLM runtime | **LM Studio + MLX** | Prompt-assist; OpenAI-compatible localhost API |
| Diffusion runtime | **ComfyUI (MPS/MLX)** or Draw Things | Image + video generation on Apple Silicon |
| Image models | **Flux.1** (prose) / **Pony V6/V7 / Illustrious SDXL** (booru tags) / **RealVisXL V5** (photoreal) + adult LoRAs | The actual image *generators*; pick by style (see §6.1) |
| Face-swap | **InsightFace + InSwapper** (ONNX/CoreML) + GFPGAN/CodeFormer restore | Light; runs fast on the Mac |
| Video model | **LTX-Video** (fastest on Mac) or a smaller **Wan 2.2** variant | img2vid; slow on Apple Silicon — see §6 caveat |
| **Prompt-assist LLM** | **~24B abliterated** (e.g. Mistral Small 24B) on the Mac | Kept modest so it shares memory with diffusion; rationale in §6 |
| Auth / Payments / Cache / Moderation / AWS-GPU | **None** | All removed per scope |

### 2.4 Data model (only two tables left)
- `jobs` (id, type[image|swap|video], status, params_json, input_media_ids, output_media_ids, created/updated)
- `media` (id, s3_key, kind, created_at)

No users, sessions, consent, moderation, or audit tables — all removed with auth/CSAM.

---

## 3. The Four Features — How Each Works

### 3.1 Prompt → NSFW image
1. Client sends short idea → Mac API over Tailscale.
2. **Prompt enhancer** (uncensored LLM on the Mac) expands into a structured positive/negative diffusion prompt; returns preview to edit.
3. User confirms → enqueues an `image` job in the local queue.
4. Worker runs SDXL/Flux (+ adult LoRAs) via ComfyUI-MPS → saves to media dir.
5. Client polls → shows result.

### 3.2 Additional context prompt
- Second text field maps to: extra positive tokens, **negative prompt**, style preset, and optional **reference image** (IP-Adapter / img2img / ControlNet conditioning). Stored in `jobs.params_json`. Same pipeline, extra conditioning inputs.

### 3.3 Face swap
1. User uploads source face (or picks from prior gen) + target image.
2. Enqueue `swap` job → worker: InsightFace detect + InSwapper → GFPGAN/CodeFormer restore/upscale → media dir.
3. Client polls → result.

### 3.4 Image + prompt → video
1. User picks an image + motion prompt + duration/fps.
2. Enqueue `video` job (long timeout — slow on Apple Silicon).
3. Worker loads LTX-Video/Wan → frames → ffmpeg encode mp4 → media dir.
4. Progress streamed (frame %). Output capped (e.g. 2–6s, fixed resolution) in v1.

---

## 4. Cost (essentially zero ongoing)
- **No payments, no cloud GPU.** Everything runs on hardware you own, so ongoing cost ≈ **electricity** only.
- One-time / minor: Apple Developer account ($99/yr) for iOS signing; Tailscale (free tier is fine for personal use).
- Optional AWS: only if you opt into S3 + CloudFront for off-device media storage (a few dollars/month at personal scale).
- Per-job timeouts + output caps keep a runaway video job from pinning the Mac for hours.

---

## 5. Privacy & data (lightweight — no compliance machinery)
- **Nothing is publicly exposed.** The Mac has no open inbound ports; the phone reaches it only inside the **Tailscale mesh** (E2E encrypted). This is the main privacy win of going local.
- Media stays **on the Mac** by default — your generations never leave your machine unless you opt into S3.
- Optional: full-disk encryption (FileVault) on the Mac; periodic local backup of the media dir.
- That's it — no auth, audit log, watermarking, or moderation. (Re-add if you ever distribute.)

---

## 6. Prompt-Assist LLM — Research & Recommendation

**Goal:** a self-hosted LLM that takes a short user idea and expands it into a strong diffusion prompt, **without refusing NSFW content** (any hosted/commercial model — Grok, GPT, Claude, Gemini — will refuse or ban you, so this must be local + uncensored).

> ⚠️ **This LLM does not generate images or video.** It only produces *text* (the prompt). The pixels come from the diffusion models in §2.3 (Flux/SDXL/Pony for images, Wan/Hunyuan for video). This section is purely about the prompt-writing step.

### 6.1 The key fork: prose vs. tags (decide this first)
The "best" model depends on which **image** model you run:
- **Flux** → wants **natural-language prose** prompts. A general instruct LLM excels here.
- **Pony / Illustrious / anime SDXL** → want **booru-style tags** (e.g. `1girl, ...`), not prose. Here a small tag-completion model or a tag-trained LLM beats a big prose model.

### 6.2 Runtime on the Mac
- Run **LM Studio** with the **MLX** backend (Apple-native; 20–50% faster than llama.cpp/Ollama on Apple Silicon, and the only way to use unified memory well).
- It serves an **OpenAI-compatible API** (`/v1/chat/completions`) on localhost; the local Rust worker calls it directly.
- (The phone reaches the *whole backend* over Tailscale — LM Link specifically isn't needed here since the worker and LLM are on the same machine.)

### 6.3 Best LLM size — keep it modest (~24B), because it shares the Mac with diffusion
This is the key change from running the LLM alone. The Mac now also loads image/video models, so don't pin a 70GB LLM. Prompt expansion is an easy task — a mid-size model is more than enough:
| Pick | Resident size | Notes |
|---|---|---|
| **Mistral Small 24B abliterated** (MLX 4–8-bit) | ~14–24GB | **Recommended.** Excellent instruction-following, leaves 100GB+ for the diffusion/video model loaded per job. |
| **Mistral-Nemo 12B abliterated** | ~8–12GB | Lighter still; fine if you want maximum headroom. |
| Qwen3.5 122B / Llama-70B abliterated | ~70GB | Overkill *and* memory-hungry — only worth it if you drop video off the Mac. Not recommended for the all-on-Mac setup. |

**Bottom line:** **Mistral Small 24B abliterated**, kept warm. It costs ~20GB, writes great prompts, and leaves the bulk of the 128GB free for Flux/SDXL or the video model when a job runs.

### 6.4 Practical notes & caveats
- Get abliterated weights from **HuggingFace** (`abliterated`, `uncensored`, `heretic`; authors `cognitivecomputations`, `failspy`). Prefer **MLX-format** quants.
- The worker treats the LLM as just an OpenAI endpoint. If LM Studio is down, **fall back** to submitting the raw prompt so generation still works.
- **System prompt** matters more than model size: frame it as a diffusion-prompt engineer emitting style/lighting/lens/composition tokens + a negative-prompt block. Keep the result **editable in the UI**.
- **Single point of failure:** the Mac must be **on and awake** for anything to work now (it's the whole backend). Disable sleep / App Nap; set `caffeinate` or "prevent sleep" while serving.
- **Video is the slow part.** On Apple Silicon, prefer **LTX-Video** (built for speed/efficiency) or a smaller Wan 2.2 variant; expect minutes per clip and cap duration/resolution. If it's too slow, the one piece worth moving to a rented cloud GPU is video — everything else stays happily on the Mac.

> Sources: [aiproductivity — Apple M5 Max Local LLM 128GB Guide 2026](https://aiproductivity.ai/blog/apple-m5-max-local-llm-guide/) · [JMLab — Best Local LLMs for M5 MacBook](https://jmlab.net/blog/2026-03-29-best-local-llms-m5-macbook/) · [runaihome — LM Studio + LM Link 2026](https://runaihome.com/blog/lm-studio-locally-lm-link-iphone-setup-2026/) · [LM Studio Docs — OpenAI-compatible API](https://lmstudio.ai/docs/app) · [AtlasCloud — Uncensored AI Models 2026](https://www.atlascloud.ai/blog/guides/best-uncensored-ai-models)

---

## 7. ROADMAP (trackable)

> Mark progress by checking boxes. Suggested team: 1 Rust/backend, 1 JS/client (can be the same person for a personal tool). Timelines are rough.

### Phase 0 — Foundation & Setup (≈1 wk) `[ ] 0%`
- [ ] Confirm shape: pure-JS + Capacitor → APK/IPA, **everything on the M5**, Tailscale, no auth/cache/payments
- [ ] Install **Tailscale** on the Mac + phone(s); confirm phone reaches the Mac on cellular
- [ ] Set up signing: Android keystore; Apple Developer account ($99/yr) or free-ID/TestFlight for IPA
- [ ] Choose image model (Flux vs Pony/SDXL) → decides prose vs tag prompt-assist (see §6)
- [ ] Install LM Studio (MLX), ComfyUI (MPS), pull model weights; verify licenses

### Phase 1 — Backend Core (Rust API on the Mac) (≈1–2 wks) `[ ] 0%`
- [ ] Axum service skeleton, health checks, logging — bound to the Tailscale interface
- [ ] SQLite (or local Postgres) `jobs` + `media` schema + migrations
- [ ] Job submission API (returns job id) + status/poll endpoints (no auth)
- [ ] Local job queue: worker polls the `jobs` table
- [ ] Media dir + endpoint to serve generated files over Tailscale

### Phase 2 — Image Generation + Prompt Assist (≈2–3 wks) `[ ] 0%`
- [ ] Rust worker consuming the local queue
- [ ] Image model wired in via ComfyUI-MPS (SDXL/Flux behind a localhost call)
- [ ] Positive/negative/reference conditioning → `params_json` contract
- [ ] **LM Studio + MLX** running **Mistral Small 24B abliterated** kept warm; worker calls its localhost OpenAI endpoint + fallback to raw prompt if down
- [ ] End-to-end: prompt → enhanced prompt → image in gallery
- [ ] Confirm memory behavior: LLM resident + image model loaded per job fits in 128GB

### Phase 3 — Pure-JS Client + Capacitor v1 (≈3–4 wks, overlaps P1/P2) `[ ] 0%`
- [ ] Pure-JS web app scaffold (Vite build); responsive phone + tablet layouts; theming
- [ ] **Capacitor** integration → generate Android + iOS native projects
- [ ] Build pipeline: signed **APK** (Android) + **IPA** (iOS via Xcode/TestFlight)
- [ ] Capacitor plugins wired: file picker / camera (uploads), share, **Tailscale connectivity**
- [ ] Prompt screen with enhancer + additional-context/negative fields
- [ ] Job submit → live status (poll/WSS) → result viewer
- [ ] Gallery/history; media loading from the Mac
- [ ] Settings (incl. Mac/backend address)

### Phase 4 — Face Swap (≈1–2 wks) `[ ] 0%`
- [ ] Upload pipeline for source/target images
- [ ] InsightFace/InSwapper worker path (ONNX/CoreML) + GFPGAN/CodeFormer restore
- [ ] `swap` job type in the queue
- [ ] JS swap flow (pick source face + target)

### Phase 5 — Image → Video (the slow one) (≈3–4 wks) `[ ] 0%`
- [ ] Wire **LTX-Video** (or smaller Wan 2.2) into ComfyUI-MPS; benchmark real clip times on the M5
- [ ] Load/unload video model per job (it's big) — confirm it coexists with the warm LLM
- [ ] Motion prompt + duration/fps params; conservative output caps (short clips)
- [ ] ffmpeg encode to mp4
- [ ] Progress streaming (frame %) to client
- [ ] JS video flow + player
- [ ] Decision gate: if clip times are unacceptable, plan optional cloud-GPU offload for video only

### Phase 6 — Hardening & Wrap-up (≈1 wk) `[ ] 0%`
- [ ] Keep-awake (`caffeinate`/prevent sleep) so the backend stays reachable
- [ ] Local backup of `media` dir; FileVault on
- [ ] Basic logging/metrics; per-job timeouts + output caps enforced
- [ ] (Optional) S3 + CloudFront if you want off-device media
- [ ] Install on target devices → real-use validation → **done**

### Phase 7 — Iteration / v2 (ongoing) `[ ] 0%`
- [ ] Quality: better models, LoRA library, presets
- [ ] Longer/HD video, inpainting/outpainting, batch gen
- [ ] Optional cloud-GPU offload for video if the Mac is too slow

---

## 8. Top Risks & Mitigations
| Risk | Impact | Mitigation |
|---|---|---|
| iOS self-signing friction | Can't easily install IPA | Apple Developer account ($99/yr) for 1-yr provisioning, or TestFlight |
| **Mac is a single point of failure** (whole backend) | App fully down if Mac off/asleep/offline | Keep-awake (`caffeinate`); UPS optional; it's an accepted tradeoff for a personal tool |
| **Video too slow on Apple Silicon** | Long waits per clip | Use LTX-Video / small Wan, cap duration; offload video to a rented GPU if intolerable |
| Memory contention (LLM + diffusion + video) | OOM / swapping | Keep LLM ~24B; load heavy models per job, serially; never all at once |
| Model ToS violation (Grok etc.) | Account/API cutoff | Self-host open models for NSFW + prompt assist; nothing routed to hosted APIs |
| Illegal content (CSAM / non-consensual real people) | **Criminal — on you, since detection was removed** | Operator discipline: keep it synthetic/consented (see §0.3) |

---

## 9. Immediate Next Actions
1. **Pick the image model** (Flux=prose vs Pony/SDXL=tags) — this locks the prompt-assist choice in §6.
2. Install Tailscale on Mac + phone; confirm the phone reaches the Mac on cellular.
3. Install LM Studio (MLX) + ComfyUI (MPS); pull **Mistral Small 24B abliterated** + the image/video models.
4. Set up Android keystore + Apple Developer/TestFlight signing.
5. I scaffold the **Rust Axum backend (local queue + SQLite)** + pure-JS/Capacitor client shell once you confirm.

> **Progress tracking:** update the `[ ] %` next to each phase as boxes get checked. Keep this file in the repo; treat it as the single source of truth for status.
