import { api } from "../api.js";
import { appendMedia, esc } from "../ui.js";

// Supporting feature: generation history / gallery (§1.2).
export function mountGallery(view) {
  view.innerHTML = `
    <div class="card">
      <h2>History
        <button class="btn secondary" id="gl-refresh" style="width:auto;margin:0;float:right;padding:6px 12px">Refresh</button>
      </h2>
      <div id="gl-body"><p class="muted">Loading…</p></div>
    </div>
  `;

  const body = view.querySelector("#gl-body");

  async function load() {
    body.innerHTML = '<p class="muted">Loading…</p>';
    try {
      const jobs = await api.listJobs();
      const done = jobs.filter(
        (j) => j.status === "done" && j.output_media_ids.length,
      );
      if (!done.length) {
        body.innerHTML =
          '<p class="muted">No generations yet. Make something in Generate / Swap / Video.</p>';
        return;
      }
      body.innerHTML = "";
      const grid = document.createElement("div");
      grid.className = "gallery-grid";
      done.forEach((j) => {
        const item = document.createElement("div");
        item.className = "gallery-item blurred";

        // Eye toggle — reveal/hide the image
        const eyeBtn = document.createElement("button");
        eyeBtn.className = "eye-btn";
        eyeBtn.innerHTML = "&#x1F441;"; // 👁
        eyeBtn.title = "Reveal image";
        eyeBtn.addEventListener("click", (e) => {
          e.stopPropagation();
          const blurred = item.classList.toggle("blurred");
          eyeBtn.innerHTML = blurred ? "&#x1F441;" : "&#x1F648;";
          eyeBtn.title = blurred ? "Reveal image" : "Hide image";
        });
        item.appendChild(eyeBtn);

        // Delete button
        const delBtn = document.createElement("button");
        delBtn.className = "del-btn";
        delBtn.innerHTML = "&#10005;";
        delBtn.title = "Delete this media";
        delBtn.addEventListener("click", async (e) => {
          e.stopPropagation();
          if (!confirm("Delete this item permanently?")) return;
          try {
            await api.deleteMedia(j.output_media_ids[0]);
            load(); // refresh the gallery
          } catch (err) {
            alert("Delete failed: " + err.message);
          }
        });
        item.appendChild(delBtn);

        appendMedia(item, j.output_media_ids[0]);
        const meta = document.createElement("div");
        meta.className = "meta";
        meta.innerHTML = `<span class="badge">${esc(j.type)}</span><span>${esc(j.created_at)}</span>`;
        item.appendChild(meta);
        grid.appendChild(item);
      });
      body.appendChild(grid);
    } catch (e) {
      body.innerHTML = `<div class="error">Could not load history: ${esc(e.message)}</div>`;
    }
  }

  view.querySelector("#gl-refresh").addEventListener("click", load);
  load();
  return null;
}
