import { useCallback, useEffect, useMemo, useState } from "react";
import { dataDir } from "@tauri-apps/api/path";
import { open, save } from "@tauri-apps/plugin-dialog";
import Toast from "../../components/Toast";
import { isAppError, type AppError } from "../../lib/appError";
import { APP_DATA_DIR_NAME } from "../../lib/imagePath";
import {
  presetsExport,
  presetsGet,
  presetsImport,
  presetsSave,
  type Preset,
  type PresetSnapshot,
} from "../../lib/tauri";
import { presetLabel } from "../generate/presetTypes";
import ABCompareModal from "./ABCompareModal";
import { isFormValid, toFormValues, toSavePayload, type PresetFormValues } from "./presetForm";

const inputClass =
  "w-full rounded-md border border-border bg-surface-2 px-3 py-2 text-sm text-text placeholder:text-text-sub focus:border-primary focus:outline-none";

/**
 * 프리셋 편집기 (04 §4.4, T5.1/T5.2/T5.3): 좌 리스트(버전 배지) / 우 편집 폼 +
 * A/B 비교 + 내보내기/가져오기.
 */
export default function PresetsScreen() {
  const [presets, setPresets] = useState<Preset[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<AppError | null>(null);
  const [selectedId, setSelectedId] = useState<string>("");
  const [form, setForm] = useState<PresetFormValues | null>(null);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [saving, setSaving] = useState(false);
  const [toast, setToast] = useState<{ message: string; tone: "error" | "success" } | null>(null);
  const [compareOpen, setCompareOpen] = useState(false);
  const [dataRoot, setDataRoot] = useState<string | null>(null);
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);

  useEffect(() => {
    dataDir()
      .then((dir) => setDataRoot(`${dir.replace(/\/$/, "")}/${APP_DATA_DIR_NAME}`))
      .catch(() => setDataRoot(null));
  }, []);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await presetsGet();
      setPresets(list);
      setSelectedId((current) =>
        list.some((p) => p.id === current) ? current : (list[0]?.id ?? ""),
      );
    } catch (e) {
      setError(
        isAppError(e)
          ? e
          : { code: "E_UNKNOWN", message: "프리셋을 불러오지 못했어요.", detail: String(e) },
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const selected = useMemo(
    () => presets.find((p) => p.id === selectedId) ?? null,
    [presets, selectedId],
  );

  // 선택이 바뀌면 편집 폼을 해당 프리셋 값으로 초기화 (미저장 편집은 버림)
  useEffect(() => {
    setForm(selected ? toFormValues(selected) : null);
    setHistoryOpen(false);
  }, [selected]);

  const handleSave = useCallback(async () => {
    if (!selected || !form || !isFormValid(form)) return;
    setSaving(true);
    try {
      await presetsSave(toSavePayload(selected, form));
      setToast({ message: `v${selected.version + 1}로 저장했어요.`, tone: "success" });
      await load();
    } catch (e) {
      setToast({
        message: isAppError(e) ? e.message : "프리셋을 저장하지 못했어요. 다시 시도해 주세요.",
        tone: "error",
      });
    } finally {
      setSaving(false);
    }
  }, [selected, form, load]);

  const handleRestore = useCallback(
    async (snapshot: PresetSnapshot) => {
      if (!selected) return;
      if (
        !window.confirm(`v${snapshot.version} 상태로 복원할까요? 현재 상태는 히스토리에 남아요.`)
      ) {
        return;
      }
      setSaving(true);
      try {
        await presetsSave(toSavePayload(selected, toFormValues(snapshot)));
        setToast({ message: `v${snapshot.version} 상태로 복원했어요.`, tone: "success" });
        await load();
      } catch (e) {
        setToast({
          message: isAppError(e) ? e.message : "프리셋을 복원하지 못했어요. 다시 시도해 주세요.",
          tone: "error",
        });
      } finally {
        setSaving(false);
      }
    },
    [selected, load],
  );

  const handleExport = useCallback(async () => {
    const destPath = await save({
      title: "프리셋 내보내기",
      defaultPath: "presets.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!destPath) return;
    setExporting(true);
    try {
      await presetsExport(destPath);
      setToast({ message: "프리셋을 내보냈어요.", tone: "success" });
    } catch (e) {
      setToast({
        message: isAppError(e) ? e.message : "프리셋을 내보내지 못했어요. 다시 시도해 주세요.",
        tone: "error",
      });
    } finally {
      setExporting(false);
    }
  }, []);

  const handleImport = useCallback(async () => {
    const srcPath = await open({
      title: "프리셋 가져오기",
      multiple: false,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!srcPath || Array.isArray(srcPath)) return;
    setImporting(true);
    try {
      const merged = await presetsImport(srcPath);
      setPresets(merged);
      setToast({ message: "프리셋을 가져왔어요.", tone: "success" });
    } catch (e) {
      setToast({
        message: isAppError(e) ? e.message : "프리셋을 가져오지 못했어요. 다시 시도해 주세요.",
        tone: "error",
      });
    } finally {
      setImporting(false);
    }
  }, []);

  return (
    <div className="flex h-full">
      <div className="w-56 shrink-0 overflow-y-auto border-r border-border p-4">
        <h1 className="mb-3 text-base font-medium text-text">프리셋</h1>
        {loading && <p className="text-xs text-text-sub">불러오는 중…</p>}
        {error && (
          <div role="alert" className="rounded-md border border-error bg-surface-2 px-2 py-2">
            <p className="text-xs text-error">{error.message}</p>
            <button
              type="button"
              onClick={() => void load()}
              className="mt-1 rounded-sm border border-border px-2 py-1 text-xs text-text-sub hover:bg-surface"
            >
              다시 시도
            </button>
          </div>
        )}
        <ul className="space-y-1">
          {presets.map((p) => (
            <li key={p.id}>
              <button
                type="button"
                onClick={() => setSelectedId(p.id)}
                className={`ease-out-ui flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left text-sm transition-colors duration-150 ${
                  p.id === selectedId ? "bg-primary text-white" : "text-text hover:bg-surface-2"
                }`}
              >
                <span className="truncate">{presetLabel(p)}</span>
                <span
                  className={`ml-2 shrink-0 rounded-sm px-1.5 py-0.5 text-[10px] ${
                    p.id === selectedId ? "bg-white/20" : "border border-border text-text-sub"
                  }`}
                >
                  v{p.version}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </div>

      <div className="flex-1 overflow-y-auto p-6">
        {selected && form ? (
          <div className="max-w-xl space-y-4">
            <div className="flex flex-wrap items-center gap-2">
              <button
                type="button"
                disabled={saving || !isFormValid(form)}
                onClick={() => void handleSave()}
                className="ease-out-ui rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-white transition-colors duration-150 hover:bg-primary-hover disabled:opacity-40"
              >
                버전 저장
              </button>
              <button
                type="button"
                disabled={selected.history.length === 0}
                onClick={() => setHistoryOpen((v) => !v)}
                className="ease-out-ui rounded-md border border-border px-3 py-1.5 text-xs text-text transition-colors duration-150 hover:bg-surface-2 disabled:opacity-40"
              >
                이전 버전 보기/복원 ({selected.history.length})
              </button>
              <button
                type="button"
                disabled={presets.length < 2}
                onClick={() => setCompareOpen(true)}
                title={presets.length < 2 ? "비교할 프리셋이 2개 이상 필요해요" : undefined}
                className="ease-out-ui rounded-md border border-border px-3 py-1.5 text-xs text-text transition-colors duration-150 hover:bg-surface-2 disabled:opacity-40"
              >
                A/B 비교
              </button>
              <button
                type="button"
                disabled={exporting}
                onClick={() => void handleExport()}
                className="ease-out-ui rounded-md border border-border px-3 py-1.5 text-xs text-text transition-colors duration-150 hover:bg-surface-2 disabled:opacity-40"
              >
                내보내기
              </button>
              <button
                type="button"
                disabled={importing}
                onClick={() => void handleImport()}
                className="ease-out-ui rounded-md border border-border px-3 py-1.5 text-xs text-text transition-colors duration-150 hover:bg-surface-2 disabled:opacity-40"
              >
                가져오기
              </button>
            </div>

            {historyOpen && (
              <div className="space-y-2 rounded-md border border-border bg-surface-2 p-3">
                {selected.history.map((snapshot) => (
                  <div
                    key={snapshot.version}
                    className="flex items-center justify-between gap-2 rounded-sm bg-surface px-2 py-1.5"
                  >
                    <div className="min-w-0">
                      <p className="text-xs text-text">v{snapshot.version}</p>
                      <p className="truncate text-xs text-text-sub">{snapshot.prefix}</p>
                    </div>
                    <button
                      type="button"
                      onClick={() => void handleRestore(snapshot)}
                      className="ease-out-ui shrink-0 rounded-sm border border-border px-2 py-1 text-xs text-text transition-colors duration-150 hover:bg-surface-2"
                    >
                      복원
                    </button>
                  </div>
                ))}
              </div>
            )}

            <label className="block space-y-1">
              <span className="text-xs text-text-sub">이름 (한국어)</span>
              <input
                className={inputClass}
                value={form.labelKo}
                onChange={(e) => setForm((f) => f && { ...f, labelKo: e.target.value })}
              />
            </label>
            <label className="block space-y-1">
              <span className="text-xs text-text-sub">이름 (영어)</span>
              <input
                className={inputClass}
                value={form.labelEn}
                onChange={(e) => setForm((f) => f && { ...f, labelEn: e.target.value })}
              />
            </label>
            <label className="block space-y-1">
              <span className="text-xs text-text-sub">prefix</span>
              <textarea
                rows={2}
                className={inputClass}
                value={form.prefix}
                onChange={(e) => setForm((f) => f && { ...f, prefix: e.target.value })}
              />
            </label>
            <label className="block space-y-1">
              <span className="text-xs text-text-sub">suffix</span>
              <textarea
                rows={2}
                className={inputClass}
                value={form.suffix}
                onChange={(e) => setForm((f) => f && { ...f, suffix: e.target.value })}
              />
            </label>
            <label className="block space-y-1">
              <span className="text-xs text-text-sub">negative</span>
              <textarea
                rows={2}
                className={inputClass}
                value={form.negative}
                onChange={(e) => setForm((f) => f && { ...f, negative: e.target.value })}
              />
            </label>
            <div className="grid grid-cols-2 gap-3">
              <label className="block space-y-1">
                <span className="text-xs text-text-sub">steps</span>
                <input
                  type="number"
                  className={inputClass}
                  value={form.steps}
                  onChange={(e) => setForm((f) => f && { ...f, steps: Number(e.target.value) })}
                />
              </label>
              <label className="block space-y-1">
                <span className="text-xs text-text-sub">cfg</span>
                <input
                  type="number"
                  step="0.1"
                  className={inputClass}
                  value={form.cfg}
                  onChange={(e) => setForm((f) => f && { ...f, cfg: Number(e.target.value) })}
                />
              </label>
              <label className="block space-y-1">
                <span className="text-xs text-text-sub">width</span>
                <input
                  type="number"
                  className={inputClass}
                  value={form.width}
                  onChange={(e) => setForm((f) => f && { ...f, width: Number(e.target.value) })}
                />
              </label>
              <label className="block space-y-1">
                <span className="text-xs text-text-sub">height</span>
                <input
                  type="number"
                  className={inputClass}
                  value={form.height}
                  onChange={(e) => setForm((f) => f && { ...f, height: Number(e.target.value) })}
                />
              </label>
            </div>
            <label className="block space-y-1">
              <span className="text-xs text-text-sub">성공 기준 메모</span>
              <textarea
                rows={2}
                className={inputClass}
                value={form.successCriteria}
                onChange={(e) => setForm((f) => f && { ...f, successCriteria: e.target.value })}
              />
            </label>
          </div>
        ) : (
          !loading && <p className="text-sm text-text-sub">프리셋을 선택해 주세요.</p>
        )}
      </div>

      {toast && <Toast message={toast.message} tone={toast.tone} onClose={() => setToast(null)} />}
      {compareOpen && (
        <ABCompareModal
          presets={presets}
          initialPresetId={selectedId}
          dataRoot={dataRoot}
          onClose={() => setCompareOpen(false)}
        />
      )}
    </div>
  );
}
