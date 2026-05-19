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
          <AlertDialogDescription>
            <span className="font-medium text-foreground">{profileName}</span>{" "}
            (<code className="text-xs">{projectId}</code>)에 연결합니다. 이
            프로파일은 운영 보호가 설정되어 있습니다. 읽기 전용이지만 운영
            데이터를 조회하게 됩니다. 계속하시겠습니까?
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel onClick={onCancel}>취소</AlertDialogCancel>
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
