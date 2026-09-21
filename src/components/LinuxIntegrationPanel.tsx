import { useEffect, useState } from "react";
import {
  currentPlatform,
  installLinuxFileManagerIntegration,
  isLinuxFileManagerIntegrationInstalled,
  uninstallLinuxFileManagerIntegration,
} from "../lib/tauri";
import "./LinuxIntegrationPanel.css";

/**
 * Lets the user opt into (or out of) Satsuma's Linux file-manager context
 * menu entries (Nautilus scripts, Nemo actions, KDE Dolphin service menu,
 * plus the standard "Open With" MIME association everywhere else). Only renders on
 * Linux; on other platforms the underlying Tauri commands are no-ops that
 * report the feature as unavailable, so this panel just stays hidden there
 * rather than showing a permanently-broken toggle.
 */
export function LinuxIntegrationPanel() {
  const [visible, setVisible] = useState(false);
  const [installed, setInstalled] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      const platform = await currentPlatform();
      if (cancelled) return;
      setVisible(platform === "linux");
      if (platform !== "linux") return;

      const status = await isLinuxFileManagerIntegrationInstalled();
      if (!cancelled) setInstalled(status);
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  async function toggle() {
    setBusy(true);
    setError(null);
    try {
      if (installed) {
        await uninstallLinuxFileManagerIntegration();
        setInstalled(false);
      } else {
        await installLinuxFileManagerIntegration();
        setInstalled(true);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  if (!visible) return null;

  return (
    <div className="linux-integration-panel" data-testid="linux-integration-panel">
      <p>
        File-manager context menu:{" "}
        <strong>{installed ? "enabled" : "disabled"}</strong>
      </p>
      <button type="button" onClick={toggle} disabled={busy} data-testid="linux-integration-toggle">
        {installed ? "Remove context menu entries" : "Add context menu entries"}
      </button>
      {error && (
        <p className="linux-integration-panel__error" data-testid="linux-integration-error">
          {error}
        </p>
      )}
    </div>
  );
}
