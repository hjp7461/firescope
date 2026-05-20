import { Compass, MousePointerClick, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";

/** 메인 영역의 빈 상태 — 프로파일 0개 vs 프로파일은 있고 비활성 두 분기. */
export function EmptyState({
  profileCount,
  onAdd,
}: {
  profileCount: number;
  onAdd: () => void;
}) {
  if (profileCount === 0) {
    return <FirstRunOnboarding onAdd={onAdd} />;
  }
  return <NoActiveSession />;
}

/** 첫 실행: 등록된 프로파일이 없는 경우 — 큰 CTA 카드. */
function FirstRunOnboarding({ onAdd }: { onAdd: () => void }) {
  return (
    <div className="flex flex-1 items-center justify-center p-8">
      <div className="w-full max-w-md rounded-lg border bg-card p-8 text-center shadow-sm">
        <div className="mx-auto mb-4 flex size-12 items-center justify-center rounded-full bg-primary/10 text-primary">
          <Compass className="size-6" />
        </div>
        <h2 className="text-base font-semibold">Firescope에 오신 것을 환영합니다</h2>
        <p className="mt-2 text-sm text-muted-foreground">
          Firestore를 안전하게 들여다보려면 먼저 프로파일을 추가하세요.
          서비스 계정 JSON 또는 로컬 에뮬레이터 호스트만 있으면 됩니다.
        </p>
        <Button className="mt-5 gap-1.5" onClick={onAdd}>
          <Plus className="size-4" />
          프로파일 추가하기
        </Button>
        <p className="mt-4 text-xs text-muted-foreground">
          자격증명은 OS 키체인/자격증명 관리자에 안전하게 저장됩니다.
        </p>
      </div>
    </div>
  );
}

/** 프로파일은 있지만 세션 비활성 — 짧은 힌트. */
function NoActiveSession() {
  return (
    <div className="flex flex-1 items-center justify-center p-8">
      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <MousePointerClick className="size-4" />
        왼쪽에서 프로파일을 더블클릭해 활성화하세요.
      </div>
    </div>
  );
}
