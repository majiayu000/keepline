import { invoke } from '@tauri-apps/api/core'
export function isNativeApp(): boolean { return '__TAURI_INTERNALS__' in window }
export async function openNativeLedger(sessionId?: string,anchor?: string) { return invoke('open_ledger',{ sessionId,anchor }) }
export async function setNativeCounts(needsYou: number,running: number) { return invoke('set_ledger_counts',{ needsYou,running }) }
export async function getAutostart() { return invoke<boolean>('get_app_autostart') }
export async function setAutostart(enabled: boolean) { return invoke('set_app_autostart',{ enabled }) }
export async function quitNativeApp(stopMonitoring: boolean) { return invoke('quit_app',{ stopMonitoring }) }
