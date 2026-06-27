// Thin API client. The backend lives on the Mac and is reached over Tailscale;
// its address is configurable in Settings and persisted to localStorage.

const KEY = "fg_backend";
const DEFAULT_BACKEND = "http://127.0.0.1:8080";

export function getBackend() {
  return localStorage.getItem(KEY) || DEFAULT_BACKEND;
}

export function setBackend(url) {
  localStorage.setItem(KEY, String(url).trim().replace(/\/+$/, ""));
}

export function initSettings() {
  if (!localStorage.getItem(KEY)) setBackend(DEFAULT_BACKEND);
}

function base() {
  return getBackend();
}

async function json(path, opts) {
  const res = await fetch(base() + path, opts);
  if (!res.ok) {
    const body = await res.text().catch(() => "");
    throw new Error(`HTTP ${res.status} ${body}`);
  }
  return res.json();
}

export const api = {
  health: () => json("/health"),

  enhance: (idea, context, negative) =>
    json("/api/prompt/enhance", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ idea, context, negative }),
    }),

  createJob: (type, params, input_media_ids = []) =>
    json("/api/jobs", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ type, params, input_media_ids }),
    }),

  getJob: (id) => json("/api/jobs/" + id),

  listJobs: () => json("/api/jobs"),

  uploadMedia: async (file) => {
    const fd = new FormData();
    fd.append("file", file);
    const res = await fetch(base() + "/api/media", {
      method: "POST",
      body: fd,
    });
    if (!res.ok) throw new Error("upload failed: HTTP " + res.status);
    return res.json(); // { id }
  },

  deleteMedia: (id) =>
    fetch(base() + "/api/media/" + id, { method: "DELETE" }).then((r) => {
      if (!r.ok) throw new Error("delete failed: HTTP " + r.status);
      return r.json();
    }),

  // --- Presets: reusable named bundles of Generate settings ---
  listPresets: (kind = "image") =>
    json("/api/presets?kind=" + encodeURIComponent(kind)),

  createPreset: (name, data, kind = "image") =>
    json("/api/presets", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ name, kind, data }),
    }),

  updatePreset: (id, body) =>
    json("/api/presets/" + id, {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    }),

  deletePreset: (id) =>
    fetch(base() + "/api/presets/" + id, { method: "DELETE" }).then((r) => {
      if (!r.ok) throw new Error("delete failed: HTTP " + r.status);
      return r.json();
    }),

  // Describe an uploaded image (vision) → detailed prompt covering selected aspects.
  describeImage: (media_id, aspects) =>
    json("/api/describe", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ media_id, aspects }),
    }),

  mediaUrl: (id) => base() + "/api/media/" + id,
};

// Poll a job once per second until it finishes. Returns a cancel function.
export function pollJob(id, onUpdate) {
  let stopped = false;
  (async () => {
    while (!stopped) {
      try {
        const job = await api.getJob(id);
        onUpdate(job);
        if (job.status === "done" || job.status === "failed") break;
      } catch (e) {
        onUpdate({ status: "error", error: String(e), progress: 0 });
      }
      await new Promise((r) => setTimeout(r, 1000));
    }
  })();
  return () => {
    stopped = true;
  };
}
