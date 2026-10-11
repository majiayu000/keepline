import { memo } from "react";
import { ExportMenu } from "@/components/ExportMenu";
import { NotificationSettings } from "@/components/NotificationSettings";
import { ConnectionStatus } from "@/components/ConnectionStatus";
import { Button } from "@/components/Button";
import type { Session } from "@/types";
import type {
  NotificationSettings as NotificationSettingsType,
  ConnectionStatus as ConnectionStatusType,
} from "@/hooks";
import styles from "./Header.module.css";

interface HeaderProps {
  onSync: () => void;
  onLogout?: () => void | Promise<void>;
  syncing?: boolean;
  sessions?: Session[];
  // Notification props
  notificationSettings?: NotificationSettingsType;
  onUpdateNotificationSettings?: (
    updates: Partial<NotificationSettingsType>,
  ) => void;
  notificationPermission?: NotificationPermission;
  onRequestNotificationPermission?: () => Promise<boolean>;
  // Connection status
  connectionStatus?: ConnectionStatusType;
}

export const Header = memo(function Header({
  onSync,
  onLogout,
  syncing = false,
  sessions = [],
  notificationSettings,
  onUpdateNotificationSettings,
  notificationPermission,
  onRequestNotificationPermission,
  connectionStatus = "polling",
}: HeaderProps) {
  return (
    <header className={styles.header} role="banner">
      <div className={styles.brand}>
        <h1 className={styles.title}>KEEPLINE</h1>
        <span className={styles.subtitle}>Agent 执行总览</span>
      </div>
      <div className={styles.actions}>
        <ConnectionStatus status={connectionStatus} />
        <ExportMenu sessions={sessions} />
        {notificationSettings &&
          onUpdateNotificationSettings &&
          onRequestNotificationPermission && (
            <NotificationSettings
              settings={notificationSettings}
              onUpdateSettings={onUpdateNotificationSettings}
              permission={notificationPermission || "default"}
              onRequestPermission={onRequestNotificationPermission}
            />
          )}
        <Button
          variant="secondary"
          size="sm"
          onClick={onSync}
          loading={syncing}
        >
          同步
        </Button>
        {onLogout && (
          <Button variant="secondary" size="sm" onClick={() => void onLogout()}>
            退出登录
          </Button>
        )}
      </div>
    </header>
  );
});
