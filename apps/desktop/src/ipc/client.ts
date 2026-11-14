/**
 * 界面到内核的唯一通道。
 *
 * 界面不直接读写文件、不开子进程、不联网。要什么数据，调内核的一个命令。
 * 组件不要直接用这里的 `call`，用 `commands.ts` 里带类型的函数，或 `queries.ts` 里的 hook。
 *
 * 数据从哪来，由运行方式决定：
 *
 * | 运行方式 | 数据 |
 * | --- | --- |
 * | `pnpm dev`、发布版（Tauri 外壳里） | 内核 |
 * | `pnpm dev:fixtures`、`pnpm screens`（浏览器，`fixtures` 模式） | `fixtures/` 里照抄原型的示例数据 |
 * | `pnpm dev:web`（浏览器，普通模式） | 没有，每个命令报 `no-shell` |
 *
 * 示例数据只在 `fixtures` 模式下加载。发布版里这段代码会被整段去掉，
 * 不会把示例数据当成真数据显示出来。
 */
import type { ErrorPayload } from "./bindings/ErrorPayload";
import type { Commands } from "./commands";

/** 内核返回的错误。按 `code` 分支，`message` 原样展示。 */
export class IpcError extends Error {
  readonly code: string;

  constructor(payload: ErrorPayload) {
    super(payload.message);
    this.name = "IpcError";
    this.code = payload.code;
  }
}

/** 把一次命令调用送到某个数据来源。出错时抛 `ErrorPayload` 形状的值。 */
export type Transport = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

function isErrorPayload(value: unknown): value is ErrorPayload {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as ErrorPayload).code === "string" &&
    typeof (value as ErrorPayload).message === "string"
  );
}

/** 页面是否跑在 Tauri 外壳里。 */
export function insideTauri(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

async function resolveTransport(): Promise<Transport> {
  if (import.meta.env.MODE === "fixtures") {
    const { fixtureTransport } = await import("./fixtures");
    return fixtureTransport;
  }
  if (insideTauri()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke;
  }
  return () =>
    Promise.reject({
      code: "no-shell",
      message: "没有连上内核。用 pnpm dev 在桌面外壳里运行，或用 pnpm dev:fixtures 看示例数据。",
    } satisfies ErrorPayload);
}

let transport: Promise<Transport> | undefined;

export async function call<K extends keyof Commands>(
  command: K,
  ...args: Parameters<Commands[K]>
): Promise<ReturnType<Commands[K]>> {
  transport ??= resolveTransport();
  try {
    const send = await transport;
    return (await send(command, args[0])) as ReturnType<Commands[K]>;
  } catch (error) {
    if (isErrorPayload(error)) throw new IpcError(error);
    throw new IpcError({ code: "internal", message: String(error) });
  }
}
