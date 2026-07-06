#!/usr/bin/env python3
"""한→영 키워드 변환 — Argos Translate 래퍼 (TAD §2, §4 ②).

계약 (CLAUDE.md: stdin 입력 → stdout JSON 한 줄, 로그는 stderr):
  입력(stdin 한 줄): {"text": "통나무집"}
  출력(stdout 한 줄):
    성공  {"ok": true, "translated": "log cabin", "engine": "argos"}
    실패  {"ok": false, "error": "<코드>", "detail": "..."}
  에러 코드: bad_input | argos_unavailable | no_ko_en_model | translate_failed

Argos가 venv에 없거나 ko→en 모델이 미설치면 실패를 반환하고,
호출 측(Rust)이 원문 폴백(§4 ③)을 처리한다.
"""

import json
import sys


def emit(obj):
    print(json.dumps(obj, ensure_ascii=False))
    sys.exit(0)


def main():
    try:
        req = json.loads(sys.stdin.readline() or "{}")
    except json.JSONDecodeError as e:
        emit({"ok": False, "error": "bad_input", "detail": str(e)})

    text = (req.get("text") or "").strip()
    if not text:
        emit({"ok": False, "error": "bad_input", "detail": "empty text"})

    try:
        from argostranslate import translate
    except ImportError as e:
        emit({"ok": False, "error": "argos_unavailable", "detail": str(e)})

    try:
        languages = translate.get_installed_languages()
        ko = next((l for l in languages if l.code == "ko"), None)
        en = next((l for l in languages if l.code == "en"), None)
        translation = ko.get_translation(en) if ko and en else None
        if translation is None:
            emit({"ok": False, "error": "no_ko_en_model", "detail": "ko->en model not installed"})
        emit({"ok": True, "translated": translation.translate(text), "engine": "argos"})
    except Exception as e:  # Argos 내부 오류는 종류가 다양 — 폴백 경로로 넘긴다
        print(f"translate error: {e}", file=sys.stderr)
        emit({"ok": False, "error": "translate_failed", "detail": str(e)})


if __name__ == "__main__":
    main()
