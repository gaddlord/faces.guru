"""
Reference image->video microservice for faces.guru (runs on the Mac).

Implements the contract the Rust backend expects:

    POST /generate   (multipart/form-data)
        image    : the still to animate
        prompt   : motion prompt
        fps      : target fps
        duration : seconds
    => 200 video/mp4

Real path drives LTX-Video or a small Wan 2.2 variant (img2vid) — the slow part on
Apple Silicon. Until that's wired, this degrades to a Ken-Burns style pan/zoom of the
still encoded to mp4 (requires imageio[ffmpeg]) so the flow is testable.

Run:
    pip install flask pillow numpy imageio imageio-ffmpeg
    # for the real model:  pip install torch diffusers (or call ComfyUI's LTX workflow)
    python video_service.py               # listens on :5001
"""

import io
import tempfile

from flask import Flask, request, send_file, abort

app = Flask(__name__)


def _real_img2vid(image_bytes, prompt, fps, duration):
    """Hook for LTX-Video / Wan. Return mp4 bytes, or None if not yet wired."""
    # TODO: load LTX-Video (or POST a ComfyUI LTX workflow) and return encoded mp4 bytes.
    return None


def _kenburns_fallback(image_bytes, fps, duration):
    """Pan/zoom the still into a short mp4 so the pipeline works without a video model."""
    import numpy as np
    import imageio
    from PIL import Image

    img = Image.open(io.BytesIO(image_bytes)).convert("RGB")
    w, h = img.size
    frames = max(1, int(fps * duration))

    tmp = tempfile.NamedTemporaryFile(suffix=".mp4", delete=False)
    tmp.close()
    writer = imageio.get_writer(tmp.name, fps=fps, codec="libx264", quality=8)
    try:
        for i in range(frames):
            t = i / max(1, frames - 1)
            zoom = 1.0 + 0.12 * t
            cw, ch = int(w / zoom), int(h / zoom)
            x = int((w - cw) * (0.5 * t))
            y = int((h - ch) * (0.5 * t))
            frame = img.crop((x, y, x + cw, y + ch)).resize((w, h), Image.LANCZOS)
            writer.append_data(np.array(frame))
    finally:
        writer.close()

    with open(tmp.name, "rb") as f:
        return f.read()


@app.post("/generate")
def generate():
    if "image" not in request.files:
        abort(400, "expected 'image' file field")

    image_bytes = request.files["image"].read()
    prompt = request.form.get("prompt", "")
    fps = int(float(request.form.get("fps", "24")))
    duration = float(request.form.get("duration", "4"))

    mp4 = _real_img2vid(image_bytes, prompt, fps, duration)
    if mp4 is None:
        mp4 = _kenburns_fallback(image_bytes, fps, duration)

    return send_file(io.BytesIO(mp4), mimetype="video/mp4", download_name="out.mp4")


if __name__ == "__main__":
    app.run(host="127.0.0.1", port=5001)
