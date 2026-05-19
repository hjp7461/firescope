import type { ReactNode } from "react";
import { Copy, KeyRound, Pencil, Trash2 } from "lucide-react";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";

export function ProfileContextMenu({
  children,
  onEdit,
  onDuplicate,
  onSetCredential,
  onDelete,
}: {
  children: ReactNode;
  onEdit: () => void;
  onDuplicate: () => void;
  onSetCredential: () => void;
  onDelete: () => void;
}) {
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent className="w-44">
        <ContextMenuItem onSelect={onEdit}>
          <Pencil className="mr-2 size-4" />
          편집
        </ContextMenuItem>
        <ContextMenuItem onSelect={onDuplicate}>
          <Copy className="mr-2 size-4" />
          복제
        </ContextMenuItem>
        <ContextMenuItem onSelect={onSetCredential}>
          <KeyRound className="mr-2 size-4" />
          자격증명 갱신
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem
          onSelect={onDelete}
          className="text-destructive focus:text-destructive"
        >
          <Trash2 className="mr-2 size-4" />
          삭제
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}
