import { api, pollJob } from '../api.js';
import { renderJobState, previewFile } from '../ui.js';

// Feature 4: image + prompt → video (animate a still into a short clip, §3.4).
export function mountVideo(view) {
  view.innerHTML = `
    <div class="card">
      <h2>Image → video</h2>
      <label>Still image to animate</label>
      <label class="file-pick">
        Tap to pick an image
        <input id="v-image" type="file" accept="image/*" />
      </label>
      <img id="v-image-prev" class="preview" />

      <label for="v-prompt">Motion prompt</label>
      <input id="v-prompt" type="text" placeholder="slow zoom in, hair blowing in the wind" />

      <div class="row">
        <div>
          <label for="v-fps">FPS</label>
          <input id="v-fps" type="number" value="24" min="8" max="30" />
        </div>
        <div>
          <label for="v-dur">Duration (s)</label>
          <input id="v-dur" type="number" value="4" min="1" max="6" step="0.5" />
        </div>
      </div>

      <button class="btn" id="v-run">Animate</button>
      <p class="muted">Video is the slow path — expect minutes per clip on the Mac.</p>
      <p class="muted" id="v-note"></p>
    </div>
    <div class="card" id="v-result-card" style="display:none">
      <h2>Result</h2>
      <div id="v-result"></div>
    </div>
  `;

  const $ = (id) => view.querySelector(id);
  const note = $('#v-note');
  let cancel = null;

  $('#v-image').addEventListener('change', (e) =>
    previewFile(e.target.files[0], $('#v-image-prev'))
  );

  $('#v-run').addEventListener('click', async () => {
    const image = $('#v-image').files[0];
    if (!image) {
      note.textContent = 'Pick an image to animate.';
      return;
    }

    note.textContent = 'Uploading…';
    $('#v-run').disabled = true;
    try {
      const up = await api.uploadMedia(image);
      const params = {
        prompt: $('#v-prompt').value.trim(),
        fps: Number($('#v-fps').value) || 24,
        duration: Number($('#v-dur').value) || 4,
      };
      const job = await api.createJob('video', params, [up.id]);
      $('#v-result-card').style.display = 'block';
      note.textContent = '';
      if (cancel) cancel();
      cancel = pollJob(job.id, (j) => {
        renderJobState($('#v-result'), j);
        if (j.status === 'done' || j.status === 'failed' || j.status === 'error')
          $('#v-run').disabled = false;
      });
    } catch (e) {
      note.textContent = 'Animate failed: ' + e.message;
      $('#v-run').disabled = false;
    }
  });

  return () => {
    if (cancel) cancel();
  };
}
