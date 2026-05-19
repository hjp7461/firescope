import { Container, KeyRound, Ticket } from "lucide-react";
import type { ProfileMode } from "@/types";

export const MODE_LABEL: Record<ProfileMode, string> = {
  emulator: "에뮬레이터",
  service_account: "서비스 계정",
  id_token: "ID 토큰",
};

export function ModeIcon({
  mode,
  className,
}: {
  mode: ProfileMode;
  className?: string;
}) {
  switch (mode) {
    case "emulator":
      return <Container className={className} aria-label="emulator" />;
    case "service_account":
      return <KeyRound className={className} aria-label="service account" />;
    case "id_token":
      return <Ticket className={className} aria-label="id token" />;
  }
}
