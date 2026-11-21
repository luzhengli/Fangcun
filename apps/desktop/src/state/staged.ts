/**
 * 检查器里「加入同步计划」的清单改动（6.8），按项目暂存。
 *
 * 只是界面上的暂存：点「同步 · N」时交给内核 `plan_sync`，和同步一起出一份计划、留一个快照。
 * 应用后清空；没应用就关掉应用，暂存的改动也就没了，清单不动。
 */
import { create } from "zustand";
import type { ManifestEdit } from "@/ipc/bindings/ManifestEdit";

interface StagedState {
  /** 项目根目录 → 暂存的改动。 */
  edits: Record<string, ManifestEdit[]>;
  stage: (root: string, edit: ManifestEdit) => void;
  /** 撤回一处：同一种改动、同一批名字。 */
  unstage: (root: string, edit: ManifestEdit) => void;
  clear: (root: string) => void;
}

const same = (a: ManifestEdit, b: ManifestEdit) =>
  a.op === b.op && a.kind === b.kind && a.names.join("\n") === b.names.join("\n");

export const useStaged = create<StagedState>((set) => ({
  edits: {},
  stage: (root, edit) =>
    set((s) => {
      const list = (s.edits[root] ?? []).filter(
        (e) => !(e.kind === edit.kind && e.names.join("\n") === edit.names.join("\n")),
      );
      return { edits: { ...s.edits, [root]: [...list, edit] } };
    }),
  unstage: (root, edit) =>
    set((s) => ({
      edits: { ...s.edits, [root]: (s.edits[root] ?? []).filter((e) => !same(e, edit)) },
    })),
  clear: (root) =>
    set((s) => {
      const { [root]: _, ...rest } = s.edits;
      return { edits: rest };
    }),
}));

const NONE: ManifestEdit[] = [];

/** 这个项目暂存的改动。没有时返回同一个空数组，免得每次渲染都换引用。 */
export function useStagedEdits(root: string | undefined): ManifestEdit[] {
  return useStaged((s) => (root ? (s.edits[root] ?? NONE) : NONE));
}
