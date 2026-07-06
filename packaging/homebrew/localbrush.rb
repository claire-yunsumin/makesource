# Homebrew Cask 초안 (D-011 참고). 아직 활성 배포 경로가 아니다 —
# `brew install --cask`로 쓰려면 별도 tap 저장소(예: claire-yunsumin/homebrew-localbrush)에
# 이 파일을 올려야 한다. tap 저장소 생성은 별도 승인 필요 — 이 파일은 그때 쓸 참고용 초안이다.
#
# 릴리스마다 손으로 채워야 할 것: version, sha256(빌드된 .dmg의 shasum -a 256 결과), url의 버전 문자열.
cask "localbrush" do
  version "0.1.0"
  sha256 :no_check # TODO: 실제 릴리스 시 `shasum -a 256 LocalBrush_<version>_aarch64.dmg` 값으로 교체

  url "https://github.com/claire-yunsumin/makesource/releases/download/v#{version}/LocalBrush_#{version}_aarch64.dmg"
  name "LocalBrush"
  desc "로컬 AI 브랜드 그래픽 생성기 (macOS, Apple Silicon 전용)"
  homepage "https://github.com/claire-yunsumin/makesource"

  # 이미지 생성(ComfyUI/MPS)이 Apple Silicon 전제 — CLAUDE.md/README 참고
  depends_on arch: :arm64

  app "LocalBrush.app"

  zap trash: [
    "~/Library/Application Support/LocalBrush",
    "~/Library/Preferences/com.localbrush.app.plist",
    "~/Library/Saved Application State/com.localbrush.app.savedState",
  ]
end
