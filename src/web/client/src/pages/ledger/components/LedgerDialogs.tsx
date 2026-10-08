import { useEffect, useRef } from "react";
import { Segmented } from "./LedgerPrimitives";
import type { LedgerController } from "@/hooks/useLedger";
import type { RequirementItem } from "../../../../../../domain/ledger/types";
import { itemStateText } from "../presentation";
import styles from "../LedgerPage.module.css";

export function LedgerDialogs({
  controller: c,
  onOpenSession,
}: {
  controller: LedgerController;
  onOpenSession: (id: string) => void;
}) {
  const {
    prompt,
    setPrompt,
    editing,
    setEditing,
    itemEditor,
    setItemEditor,
    goals,
    busy,
    perform,
    saveWorkItem,
  } = c;
  const ref = useRef<HTMLDivElement>(null);
  const open = !!(prompt || editing || itemEditor);
  useEffect(() => {
    if (!open) return;
    const previous = document.activeElement as HTMLElement | null;
    const dialog = ref.current?.querySelector<HTMLElement>('[role="dialog"]');
    dialog?.querySelector<HTMLElement>("input,textarea,button,select")?.focus();
    const before = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        setPrompt(null);
        setEditing(null);
        setItemEditor(null);
      }
      if (e.key === "Tab") {
        const controls = [
          ...(dialog?.querySelectorAll<HTMLElement>(
            'button:not(:disabled),input:not(:disabled),textarea:not(:disabled),select:not(:disabled),[tabindex="0"]',
          ) ?? []),
        ];
        const first = controls[0],
          last = controls.at(-1);
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last?.focus();
        }
        if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first?.focus();
        }
      }
    };
    document.addEventListener("keydown", key, true);
    return () => {
      document.removeEventListener("keydown", key, true);
      document.body.style.overflow = before;
      previous?.focus();
    };
  }, [open, setPrompt, setEditing, setItemEditor]);
  return (
    <div className={styles.page} ref={ref}>
      {prompt && (
        <div
          className={`${styles.dialog} ${prompt.dispatch ? styles.dispatchDialog : ""}`}
          role="dialog"
          aria-modal="true"
          aria-label={
            prompt.dispatch ? "预览交给 agent 的任务" : "编辑后续提示"
          }
        >
          <div className={styles.card}>
            <div className={styles.header}>
              <h3>
                {prompt.dispatch
                  ? `交给 ${prompt.runtime === "codex" ? "Codex" : "Claude Code"} · 先看要发的内容`
                  : "复制提示"}
              </h3>
              {prompt.dispatch && (
                <Segmented
                  label="派发 agent"
                  value={prompt.runtime ?? "codex"}
                  options={[
                    ["codex", "Codex"],
                    ["claude-code", "Claude Code"],
                  ]}
                  onChange={(runtime) => setPrompt({ ...prompt, runtime })}
                />
              )}
            </div>
            {prompt.dispatch && (
              <p className={styles.subtle}>
                待办的验收清单会成为这个会话的要求项；进度按清单逐项核对证据。
              </p>
            )}
            <textarea
              rows={14}
              value={prompt.text}
              onChange={(e) => setPrompt({ ...prompt, text: e.target.value })}
            />
            <div className={styles.actions}>
              <button
                onClick={() =>
                  void perform(() => navigator.clipboard.writeText(prompt.text))
                }
              >
                复制
              </button>
              {prompt.sessionId && (
                <button
                  onClick={() =>
                    void perform(async () => {
                      await navigator.clipboard.writeText(prompt.text);
                      onOpenSession(prompt.sessionId!);
                      setPrompt(null);
                    })
                  }
                >
                  复制并打开 agent
                </button>
              )}
              {prompt.dispatch && (
                <button
                  disabled={
                    busy || !prompt.dispatch.projectRoot || !prompt.text.trim()
                  }
                  onClick={() => void perform(c.dispatchPrompt)}
                >
                  在{" "}
                  {prompt.dispatch.projectRoot?.split("/").pop() ||
                    "未设置目录"}{" "}
                  启动 {prompt.runtime === "codex" ? "Codex" : "Claude Code"}
                </button>
              )}
              <button onClick={() => setPrompt(null)}>关闭</button>
            </div>
          </div>
        </div>
      )}
      {editing && (
        <div
          className={styles.dialog}
          role="dialog"
          aria-modal="true"
          aria-label="编辑目标或待办"
        >
          <div className={`${styles.card} ${styles.form}`}>
            <h3>
              {editing.id ? "编辑" : "新建"}{" "}
              {editing.level === "goal" ? "目标" : "待办"}
            </h3>
            <label>
              标题
              <input
                value={editing.title}
                onChange={(e) =>
                  setEditing({ ...editing, title: e.target.value })
                }
              />
            </label>
            <label>
              项目目录
              <input
                value={editing.projectRoot}
                onChange={(e) =>
                  setEditing({ ...editing, projectRoot: e.target.value })
                }
              />
            </label>
            {editing.level === "goal" ? (
              <label>
                成功标准
                <textarea
                  value={editing.outcome}
                  onChange={(e) =>
                    setEditing({ ...editing, outcome: e.target.value })
                  }
                />
              </label>
            ) : (
              <>
                <label>
                  目标
                  <select
                    value={editing.parentId}
                    onChange={(e) =>
                      setEditing({ ...editing, parentId: e.target.value })
                    }
                  >
                    <option value="">不关联目标</option>
                    {goals.map((g) => (
                      <option value={g.id} key={g.id}>
                        {g.title}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  验收清单（每行一项）
                  <textarea
                    rows={8}
                    value={editing.checklist}
                    onChange={(e) =>
                      setEditing({ ...editing, checklist: e.target.value })
                    }
                  />
                </label>
              </>
            )}
            <div className={styles.actions}>
              <button
                disabled={!editing.title.trim() || busy}
                onClick={() => void perform(saveWorkItem)}
              >
                保存
              </button>
              <button onClick={() => setEditing(null)}>取消</button>
            </div>
          </div>
        </div>
      )}
      {itemEditor !== null && (
        <div
          className={styles.dialog}
          role="dialog"
          aria-modal="true"
          aria-label="编辑并确认要求"
        >
          <div className={`${styles.card} ${styles.form}`}>
            <h3>编辑并确认要求</h3>
            <p className={styles.subtle}>
              可用路径、命令和关键词关联实际步骤。保存即确认这些要求，已删除的项不会重新出现。
            </p>
            {itemEditor.map((item, index) => {
              const update = (patch: Partial<RequirementItem>) =>
                setItemEditor((items) =>
                  items!.map((value, i) =>
                    i === index ? { ...value, ...patch } : value,
                  ),
                );
              const lines = (text: string) => text.split("\n");
              return (
                <fieldset className={styles.item} key={item.id}>
                  <legend>要求 {index + 1}</legend>
                  <label>
                    要求标题
                    <input
                      value={item.title}
                      onChange={(e) => update({ title: e.target.value })}
                    />
                  </label>
                  <label>
                    状态
                    <select
                      value={item.status}
                      onChange={(e) =>
                        update({
                          status: e.target.value as RequirementItem["status"],
                        })
                      }
                    >
                      {["todo", "doing", "done", "unverified"].map((status) => (
                        <option key={status} value={status}>
                          {itemStateText[status as keyof typeof itemStateText]}
                        </option>
                      ))}
                    </select>
                  </label>
                  {!!item.anchors.legacyCommands?.length && (
                    <p className={styles.subtle}>
                      原有匹配模式保留用于关联：{item.anchors.legacyCommands.join("、")}。验收请确认下面的完整命令。
                    </p>
                  )}
                  {(["paths", "commands", "keywords"] as const).map((kind) => (
                    <label key={kind}>
                      {kind === "paths"
                        ? "文件路径或匹配模式"
                        : kind === "commands"
                          ? "验收命令（完整文本）"
                          : "关键词"}
                      （每行一个）
                      <textarea
                        rows={2}
                        value={item.anchors[kind].join("\n")}
                        onChange={(e) =>
                          update({
                            anchors: {
                              ...item.anchors,
                              [kind]: lines(e.target.value),
                            },
                          })
                        }
                      />
                    </label>
                  ))}
                  {item.anchors.commandFormat === "legacy-unconfirmed" && (
                    <button
                      disabled={!item.anchors.commands.some(command => command.trim())}
                      onClick={() => update({ anchors: { ...item.anchors, commandFormat: "literal-v2" } })}
                    >
                      确认这些是完整验收命令
                    </button>
                  )}
                  <label className={styles.toggle}>
                    <input
                      type="checkbox"
                      checked={item.constraints.some(
                        (c) => c.kind === "no_public_api_change",
                      )}
                      onChange={(e) =>
                        update({
                          constraints: [
                            ...item.constraints.filter(
                              (c) => c.kind !== "no_public_api_change",
                            ),
                            ...(e.target.checked
                              ? [{ kind: "no_public_api_change" as const }]
                              : []),
                          ],
                        })
                      }
                    />
                    保持公共接口不变
                  </label>
                  <label>
                    禁止修改的路径（每行一个）
                    <textarea
                      rows={2}
                      value={item.constraints
                        .filter((c) => c.kind === "path_forbidden")
                        .map((c) => c.value)
                        .join("\n")}
                      onChange={(e) =>
                        update({
                          constraints: [
                            ...item.constraints.filter(
                              (c) => c.kind !== "path_forbidden",
                            ),
                            ...lines(e.target.value).map((value) => ({
                              kind: "path_forbidden" as const,
                              value,
                            })),
                          ],
                        })
                      }
                    />
                  </label>
                  <label>
                    需要保留的标签或文字（每行一个）
                    <textarea
                      rows={2}
                      value={item.constraints
                        .filter((c) => c.kind === "preserve_text")
                        .map((c) => c.value)
                        .join("\n")}
                      onChange={(e) =>
                        update({
                          constraints: [
                            ...item.constraints.filter(
                              (c) => c.kind !== "preserve_text",
                            ),
                            ...lines(e.target.value).map((value) => ({
                              kind: "preserve_text" as const,
                              value,
                            })),
                          ],
                        })
                      }
                    />
                  </label>
                  <button
                    onClick={() =>
                      setItemEditor((items) =>
                        items!.filter((_, i) => i !== index),
                      )
                    }
                  >
                    删除这项要求
                  </button>
                </fieldset>
              );
            })}
            <button
              onClick={() =>
                setItemEditor((items) => [
                  ...items!,
                  {
                    id: crypto.randomUUID(),
                    ordinal: items!.length,
                    title: "",
                    anchors: { paths: [], commands: [], keywords: [], commandFormat: "literal-v2" },
                    constraints: [],
                    source: "user",
                    status: "todo",
                    statusSource: "user",
                    evidenceIds: [],
                  },
                ])
              }
            >
              补充要求
            </button>
            <div className={styles.actions}>
              <button
                disabled={busy || itemEditor.some((item) => !item.title.trim() || item.anchors.commandFormat === "legacy-unconfirmed" && item.anchors.commands.some(command => command.trim()))}
                onClick={() => void perform(c.saveRequirements)}
              >
                保存并确认
              </button>
              <button onClick={() => setItemEditor(null)}>取消</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
