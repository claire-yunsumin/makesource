import { useCallback, useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Toast from "../../components/Toast";
import { isAppError } from "../../lib/appError";
import { joinImagePath } from "../../lib/imagePath";
import {
  GEN_DONE_EVENT,
  GEN_ERROR_EVENT,
  GEN_PROGRESS_EVENT,
  generate,
  type GenDoneEvent,
  type GenErrorEvent,
  type GenProgressEvent,
  type Preset,
} from "../../lib/tauri";
import {
  INITIAL_SESSION,
  applyDone,
  applyError,
  applyProgress,
  failLocal,
  startSession,
  type GenSession,
} from "../generate/genSession";
import { presetLabel } from "../generate/presetTypes";
import { resolveComparisonSeed } from "./abCompare";

interface ABCompareModalProps {
  presets: Preset[];
  initialPresetId: string;
  dataRoot: string | null;
  onClose: () => void;
}

/** 04 §4.4 A/B 비교 뷰: 동일 시드·키워드로 두 프리셋 결과를 좌우 2열로 (T5.2). */
export default function ABCompareModal({
  presets,
  initialPresetId,
  dataRoot,
  onClose,
}: ABCompareModalProps) {
  const [presetAId, setPresetAId] = useState(initialPresetId);
  const [presetBId, setPresetBId] = useState(
    presets.find((p) => p.id !== initialPresetId)?.id ?? initialPresetId,
  );
  const [keyword, setKeyword] = useState("");
  const [seedInput, setSeedInput] = useState("");
  const [sessionA, setSessionA] = useState<GenSession>(INITIAL_SESSION);
  const [sessionB, setSessionB] = useState<GenSession>(INITIAL_SESSION);
  const [toast, setToast] = useState<string | null>(null);

  const busy = sessionA.phase === "generating" || sessionB.phase === "generating";

  useEffect(() => {
    const unlistenPromises = [
      listen<GenProgressEvent>(GEN_PROGRESS_EVENT, (e) => {
        setSessionA((s) => applyProgress(s, e.payload));
        setSessionB((s) => applyProgress(s, e.payload));
      }),
      listen<GenDoneEvent>(GEN_DONE_EVENT, (e) => {
        setSessionA((s) => applyDone(s, e.payload));
        setSessionB((s) => applyDone(s, e.payload));
      }),
      listen<GenErrorEvent>(GEN_ERROR_EVENT, (e) => {
        setSessionA((s) => applyError(s, e.payload));
        setSessionB((s) => applyError(s, e.payload));
      }),
    ];
    return () => {
      void Promise.all(unlistenPromises).then((fns) => fns.forEach((fn) => fn()));
    };
  }, []);

  const handleGenerate = useCallback(async () => {
    if (!keyword.trim() || busy) return;
    const seed = resolveComparisonSeed(seedInput, () => Math.floor(Math.random() * 2 ** 31));
    if (seed === null) {
      setToast("시드는 정수만 입력할 수 있어요.");
      return;
    }
    for (const [presetId, setSession] of [
      [presetAId, setSessionA],
      [presetBId, setSessionB],
    ] as const) {
      try {
        const jobId = await generate({ presetId, keyword: keyword.trim(), count: 1, seed });
        setSession((s) => startSession(s, jobId, 1));
      } catch (e) {
        setSession((s) =>
          failLocal(
            s,
            isAppError(e) ? e : { code: "E_UNKNOWN", message: "생성을 시작하지 못했어요." },
          ),
        );
      }
    }
  }, [keyword, seedInput, presetAId, presetBId, busy]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
      <div className="flex max-h-full w-full max-w-3xl flex-col gap-4 overflow-y-auto rounded-lg bg-surface p-6 shadow-card">
        <div className="flex items-center justify-between">
          <h2 className="text-base font-medium text-text">A/B 비교</h2>
          <button
            type="button"
            onClick={onClose}
            className="rounded-sm px-2 py-1 text-xs text-text-sub hover:bg-surface-2"
          >
            닫기
          </button>
        </div>

        <div className="flex flex-wrap items-end gap-3">
          <label className="flex-1 space-y-1">
            <span className="text-xs text-text-sub">키워드</span>
            <input
              className="w-full rounded-md border border-border bg-surface-2 px-3 py-2 text-sm text-text focus:border-primary focus:outline-none"
              value={keyword}
              onChange={(e) => setKeyword(e.target.value)}
              placeholder="예: 동화같은 통나무집"
            />
          </label>
          <label className="w-32 space-y-1">
            <span className="text-xs text-text-sub">시드(선택)</span>
            <input
              className="w-full rounded-md border border-border bg-surface-2 px-3 py-2 text-sm text-text focus:border-primary focus:outline-none"
              value={seedInput}
              onChange={(e) => setSeedInput(e.target.value)}
              placeholder="랜덤"
            />
          </label>
          <button
            type="button"
            disabled={busy || !keyword.trim()}
            onClick={() => void handleGenerate()}
            className="ease-out-ui rounded-md bg-primary px-4 py-2 text-xs font-medium text-white transition-colors duration-150 hover:bg-primary-hover disabled:opacity-40"
          >
            {busy ? "생성 중…" : "생성"}
          </button>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <CompareColumn
            label="A"
            presets={presets}
            selectedId={presetAId}
            onSelect={setPresetAId}
            session={sessionA}
            dataRoot={dataRoot}
          />
          <CompareColumn
            label="B"
            presets={presets}
            selectedId={presetBId}
            onSelect={setPresetBId}
            session={sessionB}
            dataRoot={dataRoot}
          />
        </div>
        {(sessionA.seed !== null || sessionB.seed !== null) && (
          <p className="text-center text-xs text-text-sub">시드 {sessionA.seed ?? sessionB.seed}</p>
        )}
      </div>
      {toast && <Toast message={toast} tone="error" onClose={() => setToast(null)} />}
    </div>
  );
}

interface CompareColumnProps {
  label: string;
  presets: Preset[];
  selectedId: string;
  onSelect: (id: string) => void;
  session: GenSession;
  dataRoot: string | null;
}

function CompareColumn({
  label,
  presets,
  selectedId,
  onSelect,
  session,
  dataRoot,
}: CompareColumnProps) {
  const image = session.images[0];
  const abs = image && dataRoot ? joinImagePath(dataRoot, image.path) : null;
  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <span className="shrink-0 rounded-sm bg-surface-2 px-1.5 py-0.5 text-xs text-text-sub">
          {label}
        </span>
        <select
          value={selectedId}
          onChange={(e) => onSelect(e.target.value)}
          className="w-full rounded-md border border-border bg-surface-2 px-2 py-1 text-xs text-text focus:border-primary focus:outline-none"
        >
          {presets.map((p) => (
            <option key={p.id} value={p.id}>
              {presetLabel(p)}
            </option>
          ))}
        </select>
      </div>
      <div className="flex aspect-square items-center justify-center overflow-hidden rounded-lg bg-surface-2">
        {session.phase === "generating" ? (
          <p className="text-xs text-text-sub">{Math.round(session.progress * 100)}%</p>
        ) : session.phase === "error" ? (
          <p className="px-3 text-center text-xs text-error">{session.error?.message}</p>
        ) : abs ? (
          <img
            src={convertFileSrc(abs)}
            alt={`${label} 결과`}
            className="h-full w-full object-cover"
          />
        ) : (
          <span aria-hidden className="text-2xl text-text-sub">
            ✦
          </span>
        )}
      </div>
    </div>
  );
}
