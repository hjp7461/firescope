import { ShieldAlert } from "lucide-react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

// 원칙 1·운영 보호(`docs/07-profiles.md`): require_confirmation 프로파일은
// 활성화 전 명시적 확인을 받는다.
export function ProductionWarningModal({
  open,
  profileName,
  projectId,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  profileName: string;
  projectId: string;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <AlertDialog open={open} onOpenChange={(o) => !o && onCancel()}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle className="flex items-center gap-2 text-destructive">
            <ShieldAlert className="size-5" />
            운영 환경 연결 확인
          </AlertDialogTitle>
          <AlertDialogDescription asChild>
            <div className="space-y-3">
              <p>
                <span className="font-medium text-foreground">{profileName}</span>{" "}
                (<code className="text-xs">{projectId}</code>) 에 연결하려고 합니다.
              </p>
              <ul className="space-y-1 rounded-md border bg-muted/50 p-3 text-xs">
                <li>• 이 프로파일은 운영 보호가 설정되어 있습니다.</li>
                <li>
                  • Firescope는 <strong>읽기 전용</strong>이며 set/update/delete API가
                  존재하지 않습니다.
                </li>
                <li>• 그러나 운영 데이터를 조회하게 되므로 의도된 연결인지 확인하세요.</li>
              </ul>
            </div>
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel onClick={onCancel} autoFocus>
            취소
          </AlertDialogCancel>
          <AlertDialogAction
            onClick={onConfirm}
            className="bg-destructive text-white hover:bg-destructive/90"
          >
            계속 연결
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
