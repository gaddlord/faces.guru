import { api } from '../api.js';
import { appendMedia, esc } from '../ui.js';

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

  const body = view.querySelector('#gl-body');

  async function load() {
    body.innerHTML = '<p class="muted">Loading…</p>';
    try {
      const jobs = await api.listJobs();
      const done = jobs.filter((j) => j.status === 'done' && j.output_media_ids.length);
      if (!done.length) {
        body.innerHTML = '<p class="muted">No generations yet. Make something in Generate / Swap / Video.</p>';
        return;
      }
      body.innerHTML = '';
      const grid = document.createElement('div');
      grid.className = 'gallery-grid';
      done.forEach((j) => {
        const item = document.createElement('div');
        item.className = 'gallery-item';
        appendMedia(item, j.output_media_ids[0]);
        const meta = document.createElement('div');
        meta.className = 'meta';
        meta.innerHTML = `<span class="badge">${esc(j.type)}</span><span>${esc(j.created_at)}</span>`;
        item.appendChild(meta);
        grid.appendChild(item);
      });
      body.appendChild(grid);
    } catch (e) {
      body.innerHTML = `<div class="error">Could not load history: ${esc(e.message)}</div>`;
    }
  }

  view.querySelector('#gl-refresh').addEventListener('click', load);
  load();
  return null;
}
