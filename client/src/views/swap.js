import { api, pollJob } from '../api.js';
import { renderJobState, previewFile } from '../ui.js';

// Feature 3: face swap — take a face from a source image and apply it to a target (§3.3).
export function mountSwap(view) {
  view.innerHTML = `
    <div class="card">
      <h2>Face swap</h2>
      <label>Source face (the face to use)</label>
      <label class="file-pick">
        Tap to pick source image
        <input id="s-source" type="file" accept="image/*" />
      </label>
      <img id="s-source-prev" class="preview" />

      <label>Target image (where the face goes)</label>
      <label class="file-pick">
        Tap to pick target image
        <input id="s-target" type="file" accept="image/*" />
      </label>
      <img id="s-target-prev" class="preview" />

      <label style="display:flex;align-items:center;gap:8px;margin-top:14px">
        <input id="s-restore" type="checkbox" checked style="width:auto" />
        Restore / upscale face (GFPGAN/CodeFormer)
      </label>

      <button class="btn" id="s-run">Swap faces</button>
      <p class="muted" id="s-note"></p>
    </div>
    <div class="card" id="s-result-card" style="display:none">
      <h2>Result</h2>
      <div id="s-result"></div>
    </div>
  `;

  const $ = (id) => view.querySelector(id);
  const note = $('#s-note');
  let cancel = null;

  $('#s-source').addEventListener('change', (e) =>
    previewFile(e.target.files[0], $('#s-source-prev'))
  );
  $('#s-target').addEventListener('change', (e) =>
    previewFile(e.target.files[0], $('#s-target-prev'))
  );

  $('#s-run').addEventListener('click', async () => {
    const source = $('#s-source').files[0];
    const target = $('#s-target').files[0];
    if (!source || !target) {
      note.textContent = 'Pick both a source face and a target image.';
      return;
    }

    note.textContent = 'Uploading…';
    $('#s-run').disabled = true;
    try {
      const su = await api.uploadMedia(source);
      const tu = await api.uploadMedia(target);
      const job = await api.createJob('swap', { restore: $('#s-restore').checked }, [
        su.id,
        tu.id,
      ]);
      $('#s-result-card').style.display = 'block';
      note.textContent = '';
      if (cancel) cancel();
      cancel = pollJob(job.id, (j) => {
        renderJobState($('#s-result'), j);
        if (j.status === 'done' || j.status === 'failed' || j.status === 'error')
          $('#s-run').disabled = false;
      });
    } catch (e) {
      note.textContent = 'Swap failed: ' + e.message;
      $('#s-run').disabled = false;
    }
  });

  return () => {
    if (cancel) cancel();
  };
}
