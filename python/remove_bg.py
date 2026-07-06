#!/usr/bin/env python3
"""배경 제거 — rembg(u2net) 래퍼 (T2.4b, F-4.2 투명 배경).

계약 (CLAUDE.md: stdin 입력 → stdout JSON 한 줄, 로그는 stderr):
  입력(stdin 한 줄): {"src": "<원본 png>", "dest": "<출력 png>", "modelDir": "<u2net.onnx 폴더>"}
  출력(stdout 한 줄):
    성공  {"ok": true, "dest": "<출력 png>"}
    실패  {"ok": false, "error": "<코드>", "detail": "..."}
  에러 코드: bad_input | rembg_unavailable | model_missing | remove_failed

modelDir는 부트스트랩이 HF에서 받아둔 u2net.onnx 위치 — U2NET_HOME으로
지정해 rembg의 자체 다운로드(외부 네트워크)를 차단한다 (로컬 전용 원칙).
"""

import json
import os
import sys


def emit(obj):
    print(json.dumps(obj, ensure_ascii=False))
    sys.exit(0)


def main():
    try:
        req = json.loads(sys.stdin.readline() or "{}")
    except json.JSONDecodeError as e:
        emit({"ok": False, "error": "bad_input", "detail": str(e)})

    src = req.get("src") or ""
    dest = req.get("dest") or ""
    model_dir = req.get("modelDir") or ""
    if not src or not dest or not os.path.exists(src):
        emit({"ok": False, "error": "bad_input", "detail": f"src/dest 확인: {src} -> {dest}"})

    # rembg가 모델을 인터넷에서 받지 않도록 부트스트랩 모델 폴더를 지정
    if model_dir:
        os.environ["U2NET_HOME"] = model_dir
        if not os.path.exists(os.path.join(model_dir, "u2net.onnx")):
            emit({"ok": False, "error": "model_missing", "detail": model_dir})

    try:
        from rembg import remove
    except ImportError as e:
        emit({"ok": False, "error": "rembg_unavailable", "detail": str(e)})

    try:
        with open(src, "rb") as f:
            result = remove(f.read())
        os.makedirs(os.path.dirname(dest) or ".", exist_ok=True)
        with open(dest, "wb") as f:
            f.write(result)
        emit({"ok": True, "dest": dest})
    except Exception as e:
        print(f"remove_bg error: {e}", file=sys.stderr)
        emit({"ok": False, "error": "remove_failed", "detail": str(e)})


if __name__ == "__main__":
    main()
