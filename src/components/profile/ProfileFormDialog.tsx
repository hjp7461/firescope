import { useEffect, useState } from "react";
import { toast } from "sonner";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  createProfile,
  setCredential,
  testProfile,
  updateProfile,
} from "@/ipc/profile";
import { type ProfileMeta, type ProfileMode } from "@/types";
import { toKoreanMessage } from "@/lib/errorMessages";
import { MODE_LABEL } from "./mode";

type Props = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** 있으면 편집 모드. 없으면 신규 생성. */
  initial?: ProfileMeta | null;
};

export function ProfileFormDialog({ open, onOpenChange, initial }: Props) {
  const editing = !!initial;
  const [step, setStep] = useState("basic");
  const [savedId, setSavedId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [name, setName] = useState("");
  const [projectId, setProjectId] = useState("");
  const [mode, setMode] = useState<ProfileMode>("emulator");
  const [color, setColor] = useState("#3b82f6");
  const [tags, setTags] = useState("");
  const [group, setGroup] = useState("");
  const [firestoreHost, setFirestoreHost] = useState("");
  const [requireConfirmation, setRequireConfirmation] = useState(false);
  const [saJson, setSaJson] = useState("");
  const [idToken, setIdToken] = useState("");

  useEffect(() => {
    if (!open) return;
    setStep("basic");
    setBusy(false);
    setSavedId(initial?.id ?? null);
    setName(initial?.name ?? "");
    setProjectId(initial?.project_id ?? "");
    setMode(initial?.mode ?? "emulator");
    setColor(initial?.color ?? "#3b82f6");
    setTags(initial?.tags?.join(", ") ?? "");
    setGroup(initial?.group ?? "");
    setFirestoreHost(initial?.firestore_host ?? "");
    setRequireConfirmation(initial?.require_confirmation ?? false);
    setSaJson("");
    setIdToken("");
  }, [open, initial]);

  const tagList = tags
    .split(",")
    .map((t) => t.trim())
    .filter(Boolean);

  async function handleSave() {
    setBusy(true);
    try {
      let profileId: string;
      if (editing && initial) {
        const meta = await updateProfile({
          profile_id: initial.id,
          name,
          color,
          tags: tagList,
          group,
          firestore_host: firestoreHost || undefined,
          require_confirmation: requireConfirmation,
        });
        profileId = meta.id;
      } else {
        const meta = await createProfile({
          name,
          project_id: projectId,
          mode,
          color,
          tags: tagList,
          group: group || undefined,
          firestore_host: mode === "emulator" ? firestoreHost || undefined : undefined,
          require_confirmation: requireConfirmation,
        });
        profileId = meta.id;
      }

      if (mode === "service_account" && saJson.trim()) {
        await setCredential(profileId, { kind: "service_account", json: saJson });
      } else if (mode === "id_token" && idToken.trim()) {
        await setCredential(profileId, { kind: "id_token", token: idToken });
      }

      setSavedId(profileId);
      toast.success(editing ? "프로파일이 수정되었습니다" : "프로파일이 추가되었습니다");
      setStep("verify");
    } catch (err) {
      toast.error(toKoreanMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleTest() {
    if (!savedId) return;
    setBusy(true);
    try {
      const r = await testProfile(savedId);
      toast.success(`연결 OK · ${r.project_id} · ${r.latency_ms}ms`);
    } catch (err) {
      toast.error(toKoreanMessage(err));
    } finally {
      setBusy(false);
    }
  }

  const canSave = name.trim() && (editing || projectId.trim());

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{editing ? "프로파일 편집" : "프로파일 추가"}</DialogTitle>
          <DialogDescription>
            기본 정보 → 자격증명 → 검증 순으로 설정합니다.
          </DialogDescription>
        </DialogHeader>

        <Tabs value={step} onValueChange={setStep}>
          <TabsList className="grid w-full grid-cols-3">
            <TabsTrigger value="basic">1. 기본정보</TabsTrigger>
            <TabsTrigger value="credential">2. 자격증명</TabsTrigger>
            <TabsTrigger value="verify">3. 검증</TabsTrigger>
          </TabsList>

          <TabsContent value="basic" className="space-y-3 py-2">
            <div className="space-y-1.5">
              <Label htmlFor="pf-name">이름</Label>
              <Input
                id="pf-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="운영 - main"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="pf-project">프로젝트 ID</Label>
              <Input
                id="pf-project"
                value={projectId}
                onChange={(e) => setProjectId(e.target.value)}
                placeholder="my-project"
                disabled={editing}
              />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label>모드</Label>
                <Select
                  value={mode}
                  onValueChange={(v) => setMode(v as ProfileMode)}
                  disabled={editing}
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {(Object.keys(MODE_LABEL) as ProfileMode[]).map((m) => (
                      <SelectItem key={m} value={m}>
                        {MODE_LABEL[m]}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="pf-color">색상</Label>
                <Input
                  id="pf-color"
                  type="color"
                  value={color}
                  onChange={(e) => setColor(e.target.value)}
                  className="h-9 p-1"
                />
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label htmlFor="pf-tags">태그 (쉼표 구분)</Label>
                <Input
                  id="pf-tags"
                  value={tags}
                  onChange={(e) => setTags(e.target.value)}
                  placeholder="prod, main"
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="pf-group">그룹/폴더 (선택)</Label>
                <Input
                  id="pf-group"
                  value={group}
                  onChange={(e) => setGroup(e.target.value)}
                  placeholder="운영"
                />
              </div>
            </div>
            {mode === "emulator" && (
              <div className="space-y-1.5">
                <Label htmlFor="pf-host">Firestore 호스트 (선택)</Label>
                <Input
                  id="pf-host"
                  value={firestoreHost}
                  onChange={(e) => setFirestoreHost(e.target.value)}
                  placeholder="localhost:8080"
                />
              </div>
            )}
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={requireConfirmation}
                onChange={(e) => setRequireConfirmation(e.target.checked)}
              />
              활성화 시 운영 확인 모달 표시
            </label>
          </TabsContent>

          <TabsContent value="credential" className="space-y-3 py-2">
            {mode === "emulator" && (
              <p className="text-sm text-muted-foreground">
                에뮬레이터 모드는 자격증명이 필요 없습니다.
              </p>
            )}
            {mode === "service_account" && (
              <div className="space-y-1.5">
                <Label htmlFor="pf-sa">서비스 계정 JSON</Label>
                <Textarea
                  id="pf-sa"
                  value={saJson}
                  onChange={(e) => setSaJson(e.target.value)}
                  placeholder='{ "type": "service_account", ... }'
                  className="h-40 font-mono text-xs"
                />
                <p className="text-xs text-muted-foreground">
                  본문은 OS Vault에만 저장되며 화면/로그에 남지 않습니다.
                </p>
              </div>
            )}
            {mode === "id_token" && (
              <div className="space-y-1.5">
                <Label htmlFor="pf-token">ID 토큰</Label>
                <Textarea
                  id="pf-token"
                  value={idToken}
                  onChange={(e) => setIdToken(e.target.value)}
                  placeholder="eyJhbGc..."
                  className="h-28 font-mono text-xs"
                />
              </div>
            )}
            {editing && (
              <p className="text-xs text-muted-foreground">
                비워두면 기존 자격증명이 유지됩니다.
              </p>
            )}
          </TabsContent>

          <TabsContent value="verify" className="space-y-3 py-2">
            <p className="text-sm text-muted-foreground">
              저장 후 연결을 테스트할 수 있습니다.
            </p>
            <Button
              variant="secondary"
              disabled={!savedId || busy}
              onClick={handleTest}
            >
              연결 테스트
            </Button>
          </TabsContent>
        </Tabs>

        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            닫기
          </Button>
          <Button disabled={!canSave || busy} onClick={handleSave}>
            {busy ? "처리 중…" : "저장"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
