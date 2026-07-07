#!/usr/bin/env python3
"""스타일 에센스 추출 — Florence-2 서술 + WD14 태그 (T4.1, TAD §2, PRD F-2.5).

계약 (CLAUDE.md: stdin 입력 → stdout JSON 한 줄, 로그는 stderr):
  입력(stdin 한 줄): {"images": ["<png/jpg 경로>", ...], "hfHome": "<HF 캐시 폴더>"}
  출력(stdout 한 줄):
    성공  {"ok": true, "essencePrompt": "...", "tags": [...], "captions": [...]}
    실패  {"ok": false, "error": "<코드>", "detail": "..."}
  에러 코드: bad_input | deps_unavailable | analyze_failed

동작: 참조 이미지 3~10장에서
  ① WD14 태거(onnx)로 태그 추출 → 절반 이상에서 반복되는 스타일 태그 집계
  ② Florence-2(<MORE_DETAILED_CAPTION>)로 서술 캡션 수집 (사용자 다듬기 참고용)
  ③ 에센스 프롬프트 초안 = 공통 스타일 태그 우선 + 공통 일반 태그 보충
모델은 hfHome(HF 캐시)로만 다운로드 — Hugging Face 외 네트워크 없음 (규칙 1).

`--selftest`: 모델 없이 순수 로직(집계·조합)만 검증.
"""

import json
import os
import sys
from collections import Counter

WD14_REPO = "SmilingWolf/wd-v1-4-moat-tagger-v2"
FLORENCE_REPO = "microsoft/Florence-2-base"
WD14_THRESHOLD = 0.35
MAX_ESSENCE_TAGS = 14

# 스타일(형태·텍스처·컬러·구도)을 시사하는 태그 키워드 — 개체(내용) 태그보다 우선
STYLE_HINTS = (
    "background", "style", "color", "palette", "monochrome", "greyscale",
    "sketch", "lineart", "line art", "flat", "watercolor", "pastel",
    "gradient", "texture", "chibi", "pixel", "3d", "render", "no humans",
    "simple", "minimalist", "vector", "outline", "shading", "painting",
    "illustration", "cartoon", "anime", "realistic", "photorealistic",
    "vibrant", "muted", "soft", "bold", "high contrast", "limited",
)


def emit(obj):
    print(json.dumps(obj, ensure_ascii=False))
    sys.exit(0)


def log(msg):
    print(msg, file=sys.stderr, flush=True)


def is_style_tag(tag: str) -> bool:
    return any(hint in tag for hint in STYLE_HINTS)


def aggregate_tags(per_image_tags: list[list[str]]) -> list[str]:
    """절반 이상의 이미지에서 등장한 태그를 (스타일 우선, 빈도순) 정렬해 반환."""
    if not per_image_tags:
        return []
    counter = Counter(tag for tags in per_image_tags for tag in set(tags))
    min_count = max(1, (len(per_image_tags) + 1) // 2)
    common = [(tag, n) for tag, n in counter.items() if n >= min_count]
    # 스타일 태그 우선, 그다음 빈도, 그다음 이름(결정적 순서)
    common.sort(key=lambda t: (not is_style_tag(t[0]), -t[1], t[0]))
    return [tag for tag, _ in common]


def compose_essence(common_tags: list[str]) -> str:
    """에센스 프롬프트 초안: 스타일 태그 전부 + 일반 태그 보충, 최대 MAX_ESSENCE_TAGS."""
    style = [t for t in common_tags if is_style_tag(t)]
    general = [t for t in common_tags if not is_style_tag(t)]
    picked = (style + general)[:MAX_ESSENCE_TAGS]
    return ", ".join(picked)


def selftest():
    tags = aggregate_tags(
        [
            ["flat color", "simple background", "cat", "1girl"],
            ["flat color", "simple background", "dog"],
            ["flat color", "limited palette", "cat"],
        ]
    )
    # 3장 중 2장 이상: flat color(3), simple background(2), cat(2)
    assert tags[0] == "flat color", tags
    assert "simple background" in tags and "cat" in tags
    assert "dog" not in tags and "limited palette" not in tags and "1girl" not in tags
    # 스타일 태그가 개체 태그(cat)보다 앞
    assert tags.index("simple background") < tags.index("cat")
    essence = compose_essence(tags)
    assert essence.startswith("flat color"), essence
    assert len(essence.split(", ")) <= MAX_ESSENCE_TAGS
    assert aggregate_tags([]) == []
    print("selftest OK")


def onnx_providers(ort):
    """Apple Silicon에선 CoreML(ANE/GPU) 우선, 그 외/미지원 빌드는 CPU 폴백.

    (T9.8, docs/11 §P5.1) 사용 가능한 EP와 교집합을 취하므로 어떤 환경에서도
    안전하다 — CoreML 초기화가 실패하면 ORT가 다음 EP(CPU)로 내려간다.
    """
    preferred = ["CoreMLExecutionProvider", "CPUExecutionProvider"]
    available = set(ort.get_available_providers())
    return [p for p in preferred if p in available] or ["CPUExecutionProvider"]


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
    providers = onnx_providers(ort)
    log(f"WD14 실행 프로바이더: {providers}")
    session = ort.InferenceSession(model_path, providers=providers)
    return session, general_tags


def wd14_tag(session, general_tags, image):
    import numpy as np

    _, height, _, _ = session.get_inputs()[0].shape
    # 정사각 패드(흰 배경) 후 리사이즈, RGB→BGR (WD14 전처리 규약)
    side = max(image.size)
    from PIL import Image

    canvas = Image.new("RGB", (side, side), (255, 255, 255))
    canvas.paste(image, ((side - image.width) // 2, (side - image.height) // 2))
    canvas = canvas.resize((height, height), Image.BICUBIC)
    arr = np.asarray(canvas, dtype=np.float32)[:, :, ::-1][None]
    probs = session.run(None, {session.get_inputs()[0].name: arr})[0][0]
    return [name for i, name in general_tags.items() if float(probs[i]) >= WD14_THRESHOLD]


def load_florence(hf_home, force_fp32=False):
    import torch
    from transformers import AutoModelForCausalLM, AutoProcessor

    device = "mps" if torch.backends.mps.is_available() else "cpu"
    # MPS는 fp16으로 메모리 절반·속도 개선 (T9.8, docs/11 §P5.2).
    # 수치 문제가 생기면 호출부가 force_fp32로 재로드해 폴백한다.
    dtype = torch.float32 if (force_fp32 or device == "cpu") else torch.float16
    log(f"Florence-2 로드: device={device} dtype={dtype}")
    model = AutoModelForCausalLM.from_pretrained(
        FLORENCE_REPO, trust_remote_code=True, torch_dtype=dtype, cache_dir=hf_home
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
        # 모델이 fp16이면 입력도 맞춘다 (dtype 불일치 방지)
        pixel_values=inputs["pixel_values"].to(model.dtype),
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

    images = req.get("images") or []
    hf_home = req.get("hfHome") or None
    if not (3 <= len(images) <= 10):
        emit({"ok": False, "error": "bad_input", "detail": f"이미지 3~10장 필요: {len(images)}장"})
    missing = [p for p in images if not os.path.exists(p)]
    if missing:
        emit({"ok": False, "error": "bad_input", "detail": f"파일 없음: {missing[:3]}"})
    if hf_home:
        os.environ["HF_HOME"] = hf_home

    try:
        from PIL import Image
    except ImportError as e:
        emit({"ok": False, "error": "deps_unavailable", "detail": str(e)})

    try:
        log("WD14 태거 로드 중")
        session, general_tags = load_wd14(hf_home)
        per_image = []
        pil_images = []
        for path in images:
            img = Image.open(path).convert("RGB")
            pil_images.append(img)
            tags = wd14_tag(session, general_tags, img)
            per_image.append(tags)
            log(f"tags {os.path.basename(path)}: {len(tags)}개")

        log("Florence-2 로드 중 (첫 실행은 다운로드로 오래 걸릴 수 있음)")
        model, processor, device = load_florence(hf_home)
        captions = []
        fp32_retried = False
        for i, (path, img) in enumerate(zip(images, pil_images)):
            try:
                caption = florence_caption(model, processor, device, img)
            except Exception as e:
                # fp16 수치 문제 폴백 (T9.8 §P5.2): fp32로 1회 재로드 후 재시도
                if fp32_retried:
                    raise
                fp32_retried = True
                log(f"fp16 캡션 실패({e}) — fp32로 폴백")
                model, processor, device = load_florence(hf_home, force_fp32=True)
                caption = florence_caption(model, processor, device, img)
            captions.append(caption)
            log(f"[{i + 1}/{len(images)}] caption {os.path.basename(path)}: {caption[:60]}...")

        common = aggregate_tags(per_image)
        emit(
            {
                "ok": True,
                "essencePrompt": compose_essence(common),
                "tags": common,
                "captions": captions,
            }
        )
    except ImportError as e:
        emit({"ok": False, "error": "deps_unavailable", "detail": str(e)})
    except Exception as e:
        log(f"essence error: {e}")
        emit({"ok": False, "error": "analyze_failed", "detail": str(e)})


if __name__ == "__main__":
    main()
