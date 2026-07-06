#!/usr/bin/env python3
"""LoRA 학습 데이터셋 자동 캡션 — Florence-2 서술 + WD14 태그 (T6.2, TAD §2/§8).

계약 (CLAUDE.md: stdin 입력 → stdout JSON 한 줄, 로그는 stderr):
  입력(stdin 한 줄): {"dir": "<이미지 폴더>", "hfHome": "<HF 캐시 폴더>"}
  출력(stdout 한 줄):
    성공  {"ok": true, "items": [{"file": "img-000.png", "caption": "..."}, ...]}
    실패  {"ok": false, "error": "<코드>", "detail": "..."}
  에러 코드: bad_input | deps_unavailable | analyze_failed

캡션 = Florence-2(<MORE_DETAILED_CAPTION>) 서술 + WD14 태그(essence.py와 같은
모델·전처리). 사용자가 캡션 테이블에서 다듬거나 트리거 단어를 일괄
찾아바꾸기로 넣기 쉽도록 쉼표로 이어붙인 한 줄로 만든다(04 §4.3 ②).
개별 이미지 실패는 스킵하고 빈 캡션으로 계속 진행한다(한 장 때문에 30장
전체가 실패하면 안 됨).

모델은 hfHome(HF 캐시)로만 다운로드 — Hugging Face 외 네트워크 없음 (규칙 1).

`--selftest`: 모델 없이 순수 로직(이미지 파일 목록 필터링)만 검증.
"""

import json
import os
import sys

WD14_REPO = "SmilingWolf/wd-v1-4-moat-tagger-v2"
FLORENCE_REPO = "microsoft/Florence-2-base"
WD14_THRESHOLD = 0.35
IMAGE_EXTS = {".png", ".jpg", ".jpeg", ".webp"}


def emit(obj):
    print(json.dumps(obj, ensure_ascii=False))
    sys.exit(0)


def log(msg):
    print(msg, file=sys.stderr, flush=True)


def list_images(dir_path):
    return sorted(
        name
        for name in os.listdir(dir_path)
        if os.path.splitext(name)[1].lower() in IMAGE_EXTS
    )


def compose_caption(sentence: str, tags: list[str]) -> str:
    tag_text = ", ".join(tags)
    if sentence and tag_text:
        return f"{sentence}, {tag_text}"
    return sentence or tag_text


def selftest():
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        for name in ["b.png", "a.jpg", "notes.txt", "c.WEBP"]:
            open(os.path.join(tmp, name), "w").close()
        assert list_images(tmp) == ["a.jpg", "b.png", "c.WEBP"], list_images(tmp)

    assert compose_caption("a cat", ["flat color", "simple background"]) == (
        "a cat, flat color, simple background"
    )
    assert compose_caption("a cat", []) == "a cat"
    assert compose_caption("", ["flat color"]) == "flat color"
    assert compose_caption("", []) == ""
    print("selftest OK")


def load_wd14(hf_home):
    from huggingface_hub import hf_hub_download
    import onnxruntime as ort

    model_path = hf_hub_download(WD14_REPO, "model.onnx", cache_dir=hf_home)
    csv_path = hf_hub_download(WD14_REPO, "selected_tags.csv", cache_dir=hf_home)

    import csv

    general_tags = {}
    with open(csv_path, newline="", encoding="utf-8") as f:
        for i, row in enumerate(csv.DictReader(f)):
            if row["category"] == "0":  # general
                general_tags[i] = row["name"].replace("_", " ")
    session = ort.InferenceSession(model_path, providers=["CPUExecutionProvider"])
    return session, general_tags


def wd14_tag(session, general_tags, image):
    import numpy as np
    from PIL import Image

    _, height, _, _ = session.get_inputs()[0].shape
    side = max(image.size)
    canvas = Image.new("RGB", (side, side), (255, 255, 255))
    canvas.paste(image, ((side - image.width) // 2, (side - image.height) // 2))
    canvas = canvas.resize((height, height), Image.BICUBIC)
    arr = np.asarray(canvas, dtype=np.float32)[:, :, ::-1][None]
    probs = session.run(None, {session.get_inputs()[0].name: arr})[0][0]
    return [name for i, name in general_tags.items() if float(probs[i]) >= WD14_THRESHOLD]


def load_florence(hf_home):
    import torch
    from transformers import AutoModelForCausalLM, AutoProcessor

    device = "mps" if torch.backends.mps.is_available() else "cpu"
    model = AutoModelForCausalLM.from_pretrained(
        FLORENCE_REPO, trust_remote_code=True, torch_dtype=torch.float32, cache_dir=hf_home
    ).to(device)
    processor = AutoProcessor.from_pretrained(
        FLORENCE_REPO, trust_remote_code=True, cache_dir=hf_home
    )
    return model, processor, device


def florence_caption(model, processor, device, image):
    task = "<MORE_DETAILED_CAPTION>"
    inputs = processor(text=task, images=image, return_tensors="pt").to(device)
    ids = model.generate(
        input_ids=inputs["input_ids"],
        pixel_values=inputs["pixel_values"],
        max_new_tokens=192,
        num_beams=3,
        do_sample=False,
    )
    text = processor.batch_decode(ids, skip_special_tokens=False)[0]
    parsed = processor.post_process_generation(text, task=task, image_size=image.size)
    return (parsed.get(task) or "").strip()


def main():
    if "--selftest" in sys.argv:
        selftest()
        return

    try:
        req = json.loads(sys.stdin.readline() or "{}")
    except json.JSONDecodeError as e:
        emit({"ok": False, "error": "bad_input", "detail": str(e)})

    dir_path = req.get("dir")
    hf_home = req.get("hfHome") or None
    if not dir_path or not os.path.isdir(dir_path):
        emit({"ok": False, "error": "bad_input", "detail": f"폴더 없음: {dir_path}"})

    files = list_images(dir_path)
    if not files:
        emit({"ok": False, "error": "bad_input", "detail": "이미지가 없어요."})
    if hf_home:
        os.environ["HF_HOME"] = hf_home

    try:
        from PIL import Image
    except ImportError as e:
        emit({"ok": False, "error": "deps_unavailable", "detail": str(e)})

    try:
        log("WD14 태거 로드 중")
        wd14_session, general_tags = load_wd14(hf_home)
        log("Florence-2 로드 중 (첫 실행은 다운로드로 오래 걸릴 수 있음)")
        florence_model, florence_processor, device = load_florence(hf_home)
    except ImportError as e:
        emit({"ok": False, "error": "deps_unavailable", "detail": str(e)})
    except Exception as e:
        emit({"ok": False, "error": "analyze_failed", "detail": str(e)})

    items = []
    for name in files:
        path = os.path.join(dir_path, name)
        try:
            img = Image.open(path).convert("RGB")
            tags = wd14_tag(wd14_session, general_tags, img)
            sentence = florence_caption(florence_model, florence_processor, device, img)
            caption = compose_caption(sentence, tags)
        except Exception as e:
            log(f"캡션 실패 {name}: {e}")
            caption = ""
        items.append({"file": name, "caption": caption})
        log(f"캡션 {name}: {caption[:60]}")

    emit({"ok": True, "items": items})


if __name__ == "__main__":
    main()
