import { useCallback, useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import Toast from "../../components/Toast";
import {
  cacheClear,
  cacheStats,
  licensesGet,
  modelsList,
  settingsGet,
  settingsSave,
  type AppSettings,
  type LicenseEntry,
  type ModelEntry,
} from "../../lib/tauri";
import { formatBytes, groupModels, totalBytes } from "./settingsView";

const buttonClass =
  "rounded-md px-3 py-1.5 text-sm font-medium transition-colors ease-out-ui focus:outline-none focus-visible:ring-2 focus-visible:ring-primary";

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="space-y-3">
      <h2 className="text-base font-medium text-text">{title}</h2>
      <div className="rounded-lg bg-surface p-4 shadow-card">{children}</div>
    </section>
  );
}

/**
 * 설정 화면 (04 §4.5, T7.1): 모델 관리(목록·용량), 캐시 정리, 안전 네거티브,
 * 정보(버전·오픈소스 라이선스 BOM).
 * 저장 위치 변경(F-5.2)과 업데이트 확인(F-5.4)은 후속 태스크에서.
 */
export default function SettingsScreen() {
  const [models, setModels] = useState<ModelEntry[] | null>(null);
  const [cacheBytes, setCacheBytes] = useState<number | null>(null);
  const [clearing, setClearing] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [savedNegative, setSavedNegative] = useState("");
  const [saving, setSaving] = useState(false);
  const [licenses, setLicenses] = useState<LicenseEntry[]>([]);
  const [version, setVersion] = useState("");
  const [toast, setToast] = useState<{ message: string; tone: "error" | "success" } | null>(null);

  const load = useCallback(() => {
    modelsList()
      .then(setModels)
      .catch(() => setModels([]));
    cacheStats()
      .then((s) => setCacheBytes(s.sizeBytes))
      .catch(() => setCacheBytes(null));
    settingsGet()
      .then((s) => {
        setSettings(s);
        setSavedNegative(s.safeNegative);
      })
      .catch(() => setSettings(null));
    licensesGet()
      .then(setLicenses)
      .catch(() => setLicenses([]));
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(""));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  async function onClearCache() {
    setClearing(true);
    try {
      const { freedBytes } = await cacheClear();
      setToast({ message: `캐시를 비웠어요 · ${formatBytes(freedBytes)} 확보`, tone: "success" });
      setCacheBytes(0);
    } catch {
      setToast({ message: "캐시를 정리하지 못했어요. 다시 시도해 주세요.", tone: "error" });
    } finally {
      setClearing(false);
      setConfirmClear(false);
    }
  }

  async function onSaveSettings() {
    if (!settings) return;
    setSaving(true);
    try {
      await settingsSave(settings);
      setSavedNegative(settings.safeNegative);
      setToast({ message: "설정을 저장했어요.", tone: "success" });
    } catch {
      setToast({ message: "설정을 저장하지 못했어요. 다시 시도해 주세요.", tone: "error" });
    } finally {
      setSaving(false);
    }
  }

  const groups = models ? groupModels(models) : [];

  return (
    <div className="mx-auto max-w-2xl space-y-8 overflow-y-auto p-6">
      <h1 className="text-xl font-semibold text-text">설정</h1>

      <Section title="모델 관리">
        {models === null ? (
          <p className="text-sm text-text-sub">불러오는 중…</p>
        ) : models.length === 0 ? (
          <p className="text-sm text-text-sub">
            아직 설치된 모델이 없어요. 첫 설치가 끝나면 여기에 표시돼요.
          </p>
        ) : (
          <div className="space-y-4">
            <p className="text-sm text-text-sub">
              전체 사용량{" "}
              <span className="font-medium text-text">{formatBytes(totalBytes(models))}</span>
            </p>
            {groups.map((group) => (
              <div key={group.category}>
                <h3 className="mb-1 text-xs font-medium text-text-sub">
                  {group.label} · {formatBytes(group.totalBytes)}
                </h3>
                <ul className="divide-y divide-border rounded-md border border-border">
                  {group.entries.map((m) => (
                    <li
                      key={`${m.category}/${m.name}`}
                      className="flex items-center justify-between px-3 py-2 text-sm"
                    >
                      <span className="truncate text-text">{m.name}</span>
                      <span className="ml-3 shrink-0 text-text-sub">
                        {formatBytes(m.sizeBytes)}
                      </span>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </div>
        )}
      </Section>

      <Section title="캐시">
        <div className="flex items-center justify-between gap-3">
          <p className="text-sm text-text">
            분석 모델 캐시{" "}
            <span className="text-text-sub">
              {cacheBytes === null ? "· 크기를 확인하지 못했어요" : `· ${formatBytes(cacheBytes)}`}
            </span>
            <span className="mt-0.5 block text-xs text-text-sub">
              스타일 분석에 쓰는 임시 모델이에요. 지워도 필요할 때 다시 내려받아요.
            </span>
          </p>
          {confirmClear ? (
            <div className="flex shrink-0 gap-2">
              <button
                type="button"
                disabled={clearing}
                onClick={() => void onClearCache()}
                className={`${buttonClass} bg-error text-white disabled:opacity-50`}
              >
                {clearing ? "비우는 중…" : "비우기"}
              </button>
              <button
                type="button"
                disabled={clearing}
                onClick={() => setConfirmClear(false)}
                className={`${buttonClass} border border-border text-text hover:bg-surface-2`}
              >
                취소
              </button>
            </div>
          ) : (
            <button
              type="button"
              disabled={clearing || cacheBytes === 0}
              onClick={() => setConfirmClear(true)}
              className={`${buttonClass} shrink-0 border border-border text-text hover:bg-surface-2 disabled:opacity-40`}
            >
              캐시 정리
            </button>
          )}
        </div>
      </Section>

      <Section title="생성">
        {settings === null ? (
          <p className="text-sm text-text-sub">설정을 불러오지 못했어요.</p>
        ) : (
          <label className="block space-y-1">
            <span className="text-xs text-text-sub">
              안전 네거티브 — 모든 생성의 네거티브 프롬프트 뒤에 붙어요
            </span>
            <textarea
              value={settings.safeNegative}
              onChange={(e) => setSettings({ ...settings, safeNegative: e.target.value })}
              rows={2}
              className="w-full rounded-md border border-border bg-surface-2 px-3 py-2 text-sm text-text placeholder:text-text-sub focus:border-primary focus:outline-none"
            />
            <div className="flex justify-end pt-1">
              <button
                type="button"
                disabled={saving || settings.safeNegative === savedNegative}
                onClick={() => void onSaveSettings()}
                className={`${buttonClass} bg-primary text-white hover:bg-primary-hover disabled:opacity-40`}
              >
                {saving ? "저장 중…" : "저장"}
              </button>
            </div>
          </label>
        )}
      </Section>

      <Section title="정보">
        <div className="space-y-4">
          <p className="text-sm text-text">
            LocalBrush <span className="text-text-sub">{version && `버전 ${version}`}</span>
          </p>
          <div>
            <h3 className="mb-1 text-xs font-medium text-text-sub">오픈소스 라이선스</h3>
            {licenses.length === 0 ? (
              <p className="text-sm text-text-sub">라이선스 목록을 불러오지 못했어요.</p>
            ) : (
              <ul className="divide-y divide-border rounded-md border border-border">
                {licenses.map((l) => (
                  <li key={l.name} className="px-3 py-2 text-sm">
                    <div className="flex items-center justify-between gap-3">
                      <span className="text-text">{l.name}</span>
                      <span className="shrink-0 text-xs text-text-sub">{l.license}</span>
                    </div>
                    <p className="mt-0.5 text-xs text-text-sub">
                      {l.role} · {l.url}
                    </p>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>
      </Section>

      {toast && <Toast message={toast.message} tone={toast.tone} onClose={() => setToast(null)} />}
    </div>
  );
}
