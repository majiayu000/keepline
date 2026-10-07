import type { LedgerController } from "@/hooks/useLedger";
import type { LedgerConfig } from "../../../../../../domain/ledger/types";
import { isNativeApp } from "../native";
import { ActionButton } from "./LedgerPrimitives";
import type { ReactNode } from "react";
import styles from "../Workspace.module.css";

function Toggle({
  title,
  on,
  onChange,
  disabled,
}: {
  title: string;
  on: boolean;
  onChange: (on: boolean) => void;
  disabled: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-label={title}
      aria-checked={on}
      disabled={disabled}
      className={styles.toggleSwitch}
      style={{ background: on ? "#4F5BD5" : "rgba(21,23,28,.08)" }}
      onClick={() => onChange(!on)}
    >
      <i style={{ left: on ? 18 : 2 }} />
    </button>
  );
}
function Row({
  title,
  desc,
  children,
}: {
  title: string;
  desc: string;
  children: ReactNode;
}) {
  return (
    <div className={styles.settingRow}>
      <span className={styles.settingCopy}>
        <span className={styles.settingTitle}>
          <span>{title}</span>
        </span>
        <span>
          <span>{desc}</span>
        </span>
      </span>
      {children}
    </div>
  );
}
function Value({ label, children }: { label: string; children: ReactNode }) {
  return (
    <details
      className={styles.settingEditor}
      onKeyDown={(e) => {
        if (e.key === "Escape") e.currentTarget.open = false;
      }}
    >
      <summary className={styles.settingValue}>
        <span>{label}</span>
      </summary>
      <div className={styles.settingPopover}>{children}</div>
    </details>
  );
}
export function LedgerSettings({
  controller: c,
}: {
  controller: LedgerController;
}) {
  const s = c.settings;
  const save = (patch: Partial<LedgerConfig>) =>
    void c.perform(() => c.saveSettings(patch));
  const toggle = (
    title: string,
    on: boolean,
    patch: (on: boolean) => Partial<LedgerConfig>,
  ) => (
    <Toggle
      title={title}
      on={on}
      disabled={c.busy}
      onChange={(value) => save(patch(value))}
    />
  );
  const number = (
    label: string,
    value: number,
    change: (n: number) => void,
  ) => (
    <input
      className={styles.settingValue}
      type="number"
      min={1}
      aria-label={label}
      value={value}
      onChange={(e) => change(Number(e.target.value))}
      onBlur={() => void c.perform(c.saveSettings)}
    />
  );
  const section = (title: string, children: ReactNode) => (
    <section>
      <h2
        className={styles.agentSectionTitle}
        style={{ color: "#15171C", marginBottom: 8 }}
      >
        <span>{title}</span>
      </h2>
      <div className={styles.tableCard}>{children}</div>
    </section>
  );
  return (
    <div className={`${styles.pageContent} ${styles.settingsContent}`}>
      {section(
        "通知",
        <>
          <Row
            title="等你审批或回答"
            desc="agent 停下来等你时发系统通知。关掉后只在面板和菜单栏里标出。"
          >
            {toggle("等你审批或回答", s.alerts.needs_input, (on) => ({
              alerts: { ...s.alerts, needs_input: on },
            }))}
          </Row>
          <Row title="偏离" desc="默认关。打开后，偏离已确认要求时发系统通知。">
            {toggle("偏离", s.alerts.off_plan, (on) => ({
              alerts: { ...s.alerts, off_plan: on },
            }))}
          </Row>
          <Row
            title="说完成但没证据"
            desc="默认关。只对真实要求项判断；只有候选项的会话不触发。"
          >
            {toggle("说完成但没证据", s.alerts.claimed_unverified, (on) => ({
              alerts: { ...s.alerts, claimed_unverified: on },
            }))}
          </Row>
          <Row
            title="停下了"
            desc={`默认关。会话超过 ${Math.round(s.stalledAfterSeconds / 60)} 分钟没有新步骤时提醒。`}
          >
            {toggle("停下了", s.alerts.stalled, (on) => ({
              alerts: { ...s.alerts, stalled: on },
            }))}
          </Row>
          <Row
            title="合并规则"
            desc={`同一会话 ${s.alertCoalesceSeconds / 60} 分钟内的多条提醒合并成一条。`}
          >
            <Value label={`${s.alertCoalesceSeconds / 60} 分钟`}>
              {number("提醒合并分钟数", s.alertCoalesceSeconds / 60, (n) =>
                c.setSettings({ ...s, alertCoalesceSeconds: n * 60 }),
              )}
            </Value>
          </Row>
          <Row
            title="专注时长"
            desc="侧栏「专注」每次静音的时长，期间只放行「等你」。"
          >
            <Value label={`${s.focus.minutes} 分钟`}>
              {number("专注分钟数", s.focus.minutes, (n) =>
                c.setSettings({ ...s, focus: { ...s.focus, minutes: n } }),
              )}
            </Value>
          </Row>
        </>,
      )}
      {section(
        "偏离检测",
        <>
          <Row
            title="敏感度"
            desc="保守：连续 6 步计划外才标偏离；敏感：连续 3 步。关闭则不判。"
          >
            <Value
              label={
                s.deviation === "off"
                  ? "关闭"
                  : s.deviation === "conservative"
                    ? "保守 · 6 步"
                    : "敏感 · 3 步"
              }
            >
              <select
                className={styles.settingValue}
                aria-label="偏离敏感度"
                value={s.deviation}
                onChange={(e) =>
                  save({
                    deviation: e.target.value as LedgerConfig["deviation"],
                  })
                }
              >
                <option value="off">关闭</option>
                <option value="conservative">保守 · 6 步</option>
                <option value="sensitive">敏感 · 3 步</option>
              </select>
            </Value>
          </Row>
          <Row
            title="约束检查"
            desc="对「不要改 X」「保留 Y」这类约束做规则检查。"
          >
            {toggle("约束检查", s.constraints, (on) => ({ constraints: on }))}
          </Row>
        </>,
      )}
      {section(
        "AI 判断",
        <>
          <Row
            title="AI 识别与拆分要求"
            desc="每条新的原话最多识别一次；关闭时只有候选项。模型不能判完成，也不能提供证据。"
          >
            {toggle("AI 识别与拆分要求", s.judge.enabled, (on) => ({
              judge: { ...s.judge, enabled: on },
            }))}
          </Row>
          <Row title="后端" desc="识别调用走哪里。">
            <Value
              label={
                {
                  "cli-claude": "Claude CLI",
                  "cli-codex": "Codex CLI",
                  local: "本地服务",
                  sdk: "SDK",
                }[s.judge.backend]
              }
            >
              <select
                className={styles.settingValue}
                aria-label="识别方式"
                value={s.judge.backend}
                onChange={(e) =>
                  save({
                    judge: {
                      ...s.judge,
                      backend: e.target
                        .value as LedgerConfig["judge"]["backend"],
                    },
                  })
                }
              >
                <option value="cli-claude">Claude CLI</option>
                <option value="cli-codex">Codex CLI</option>
                <option value="local">本地服务</option>
                <option value="sdk">SDK</option>
              </select>
            </Value>
          </Row>
        </>,
      )}
      {section(
        "待核对与保留",
        <>
          <Row
            title="待核对自动结束"
            desc="超过这个时长没处理，归入已结束，不再计入「需要你」。"
          >
            <span className={styles.settingValue}>12 小时</span>
          </Row>
          <Row
            title="记录保留"
            desc="只保留派生的进度账数据；transcript 留在 agent 自己的目录。"
          >
            <Value label={`${s.retentionDays} 天`}>
              {number("记录保留天数", s.retentionDays, (n) =>
                c.setSettings({ ...s, retentionDays: n }),
              )}
            </Value>
          </Row>
          <Row title="不监控的项目" desc="这些目录下的会话不读、不显示。">
            <Value label={`${s.exclude.projects.filter(Boolean).length} 个`}>
              <textarea
                className={styles.settingValue}
                style={{ height: 64, width: 240, whiteSpace: "pre-wrap" }}
                aria-label="不监控的项目"
                value={s.exclude.projects.join("\n")}
                onChange={(e) =>
                  c.setSettings({
                    ...s,
                    exclude: {
                      ...s.exclude,
                      projects: e.target.value.split("\n"),
                    },
                  })
                }
                onBlur={() => void c.perform(c.saveSettings)}
              />
            </Value>
          </Row>
        </>,
      )}
      <details className={styles.additionalSettings}>
        <summary>更多设置</summary>
        {section(
          "通知与检测",
          <>
            <Row title="额度或预算用尽" desc="agent 受额度限制时提醒。">
              {toggle("额度或预算用尽", s.alerts.limited, (on) => ({
                alerts: { ...s.alerts, limited: on },
              }))}
            </Row>
            <Row title="系统通知" desc="关闭后只在面板中展示提醒。">
              {toggle("系统通知", s.nativeNotifications, (on) => ({
                nativeNotifications: on,
              }))}
            </Row>
            <Row title="停滞判断" desc="超过这个时长没有新步骤，标为停下。">
              {number("停滞分钟数", s.stalledAfterSeconds / 60, (n) =>
                c.setSettings({ ...s, stalledAfterSeconds: n * 60 }),
              )}
            </Row>
            <Row title="目标停滞提醒" desc="目标超过这个天数没有推进时标记。">
              {number("目标停滞天数", s.staleGoalDays, (n) =>
                c.setSettings({ ...s, staleGoalDays: n }),
              )}
            </Row>
            <Row title="模型" desc="留空使用所选后端的默认模型。">
              <input
                className={styles.settingValue}
                aria-label="模型"
                value={s.judge.model ?? ""}
                onChange={(e) =>
                  c.setSettings({
                    ...s,
                    judge: { ...s.judge, model: e.target.value || null },
                  })
                }
                onBlur={() => void c.perform(c.saveSettings)}
              />
            </Row>
            <Row
              title="不监控的 agent"
              desc="选中的 agent 类型不读取、不展示。"
            >
              <select
                className={styles.settingValue}
                aria-label="不监控的 agent"
                multiple
                value={s.exclude.runtimes}
                onChange={(e) =>
                  save({
                    exclude: {
                      ...s.exclude,
                      runtimes: Array.from(
                        e.target.selectedOptions,
                        (o) => o.value,
                      ),
                    },
                  })
                }
              >
                <option value="codex">Codex</option>
                <option value="claude-code">Claude Code</option>
              </select>
            </Row>
          </>,
        )}
        {section(
          "Keepline",
          <>
            <Row title="进度账" desc="读取本地执行记录并整理进度。">
              {toggle("进度账", s.enabled, (on) => ({ enabled: on }))}
            </Row>
            {isNativeApp() && (
              <Row title="登录时启动" desc="登录 macOS 后自动启动菜单栏应用。">
                <Toggle
                  title="登录时启动"
                  on={c.autostart}
                  disabled={c.busy}
                  onChange={(on) => void c.perform(() => c.toggleAutostart(on))}
                />
              </Row>
            )}
            <Row title="Hook 事件" desc="安装 hook 以识别审批请求。">
              <ActionButton
                onClick={() =>
                  void c.perform(() =>
                    navigator.clipboard.writeText("keepline hooks install"),
                  )
                }
              >
                复制安装命令
              </ActionButton>
            </Row>
          </>,
        )}
      </details>
    </div>
  );
}
