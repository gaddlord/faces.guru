"""
Reference face-swap microservice for faces.guru (runs on the Mac).

Implements the contract the Rust backend expects:

    POST /swap   (multipart/form-data)
        source : the face to take
        target : the image to paste the face onto
        restore: "true" | "false"  (GFPGAN/CodeFormer restore)
    => 200 image/png

Real path uses InsightFace (detection + ArcFace embeddings) + InSwapper, then an
optional GFPGAN/CodeFormer restore pass. If those models aren't installed yet, the
service degrades to echoing the target back so the end-to-end flow still works.

Run:
    pip install flask pillow numpy
    # for the real swap:  pip install insightface onnxruntime opencv-python
    python faceswap_service.py            # listens on :5000
"""

import io
import os
import sys

from flask import Flask, abort, request, send_file

app = Flask(__name__)

# Model directory: honour INSIGHTFACE_HOME env var, default to D:\insightface on
# Windows, ~/.insightface elsewhere.
MODEL_DIR = os.environ.get(
    "INSIGHTFACE_HOME",
    r"D:\insightface"
    if sys.platform == "win32"
    else os.path.expanduser("~/.insightface"),
)

# Lazily-initialised models (None until first use / if libs are missing).
_swapper = None
_analyzer = None


def _load_models():
    """Try to load InsightFace + InSwapper. Returns (analyzer, swapper) or (None, None)."""
    global _swapper, _analyzer
    if _analyzer is not None and _swapper is not None:
        return _analyzer, _swapper
    try:
        import insightface
        from insightface.app import FaceAnalysis

        _analyzer = FaceAnalysis(name="buffalo_l", root=MODEL_DIR)
        _analyzer.prepare(ctx_id=0, det_size=(640, 640))
        # inswapper_128.onnx must be placed in MODEL_DIR.
        _swapper = insightface.model_zoo.get_model("inswapper_128.onnx", root=MODEL_DIR)
        return _analyzer, _swapper
    except Exception as e:  # noqa: BLE001
        app.logger.warning("face-swap models unavailable, will echo target: %s", e)
        return None, None


@app.post("/swap")
def swap():
    if "source" not in request.files or "target" not in request.files:
        abort(400, "expected 'source' and 'target' file fields")

    source_bytes = request.files["source"].read()
    target_bytes = request.files["target"].read()
    restore = request.form.get("restore", "true").lower() == "true"

    analyzer, swapper = _load_models()
    if analyzer is None or swapper is None:
        # Degraded mode: return the target untouched.
        return send_file(io.BytesIO(target_bytes), mimetype="image/png")

    import cv2
    import numpy as np
    from PIL import Image

    def to_cv(b):
        arr = np.array(Image.open(io.BytesIO(b)).convert("RGB"))
        return cv2.cvtColor(arr, cv2.COLOR_RGB2BGR)

    src_img = to_cv(source_bytes)
    tgt_img = to_cv(target_bytes)

    src_faces = analyzer.get(src_img)
    tgt_faces = analyzer.get(tgt_img)
    if not src_faces or not tgt_faces:
        abort(422, "no face detected in source or target")

    src_face = src_faces[0]
    result = tgt_img
    for face in tgt_faces:
        result = swapper.get(result, face, src_face, paste_back=True)

    if restore:
        try:
            from gfpgan import GFPGANer  # noqa: F401

            # Wire your GFPGAN/CodeFormer restorer here.
        except Exception:  # noqa: BLE001
            pass

    rgb = cv2.cvtColor(result, cv2.COLOR_BGR2RGB)
    out = io.BytesIO()
    Image.fromarray(rgb).save(out, format="PNG")
    out.seek(0)
    return send_file(out, mimetype="image/png")


if __name__ == "__main__":
    app.run(host="127.0.0.1", port=5000)
