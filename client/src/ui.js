import { api } from './api.js';

export function esc(s) {
  return String(s ?? '').replace(
    /[&<>"']/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c])
  );
}

// Render a media id into a container. Tries <img> first (works for png/jpg/gif);
// on error (e.g. an mp4) it falls back to a <video> element.
export function appendMedia(container, id) {
  const url = api.mediaUrl(id);
  const wrap = document.createElement('div');
  wrap.className = 'media-wrap';

  const img = document.createElement('img');
  img.className = 'media';
  img.loading = 'lazy';
  img.src = url;
  img.onerror = () => {
    const v = document.createElement('video');
    v.className = 'media';
    v.src = url;
    v.controls = true;
    v.loop = true;
    v.muted = true;
    v.autoplay = true;
    v.playsInline = true;
    img.replaceWith(v);
  };
  wrap.appendChild(img);

  const a = document.createElement('a');
  a.className = 'dl';
  a.href = url;
  a.target = '_blank';
  a.rel = 'noopener';
  a.textContent = 'open ↗';
  wrap.appendChild(a);

  container.appendChild(wrap);
}

// Render a job's current state (progress bar / error / result media) into a container.
export function renderJobState(container, job) {
  container.innerHTML = '';
  if (!job) return;

  if (job.status === 'failed' || job.status === 'error') {
    const d = document.createElement('div');
    d.className = 'error';
    d.textContent = 'Failed: ' + (job.error || 'unknown error');
    container.appendChild(d);
    return;
  }

  if (job.status !== 'done') {
    const pct = Math.round((job.progress || 0) * 100);
    container.innerHTML = `
      <div class="progress"><div class="bar" style="width:${pct}%"></div></div>
      <p class="muted">${esc(job.status)}… ${pct}%</p>`;
    return;
  }

  const grid = document.createElement('div');
  grid.className = 'result-grid';
  (job.output_media_ids || []).forEach((id) => appendMedia(grid, id));
  container.appendChild(grid);
}

// Show a small image preview for a chosen File in an <img> element.
export function previewFile(file, imgEl) {
  if (!file) {
    imgEl.removeAttribute('src');
    imgEl.style.display = 'none';
    return;
  }
  const reader = new FileReader();
  reader.onload = () => {
    imgEl.src = reader.result;
    imgEl.style.display = 'block';
  };
  reader.readAsDataURL(file);
}
