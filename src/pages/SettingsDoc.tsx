import { ShortcutsPanel, StoragePanel } from "./ShortcutsPage";
import { BehaviorPanel, MachinePanel } from "./PreferencesPage";
import { ThemesPanel } from "./ThemesPage";

/** 「时间线」布局里，所有设一次就不再动的东西收进这一页，从上往下读 */
export function SettingsDoc() {
  return (
    <div className="settings-doc">
      <ShortcutsPanel />
      <StoragePanel />
      <BehaviorPanel />
      <MachinePanel />
      <ThemesPanel />
    </div>
  );
}
