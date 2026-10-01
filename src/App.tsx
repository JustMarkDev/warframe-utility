import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import "./App.css";

interface SyndicateState {
  faction_key: string;
  standing: i32;
  rank: i32;
}

type i32 = number;

interface InitData {
  standings: SyndicateState[];
  authenticated: boolean;
  account_name: string | null;
  auth_refresh_required: boolean;
}

interface LogEntry {
  timestamp: string;
  message: string;
  type: "success" | "error" | "info";
}

const isAuthExpiredError = (error: any) => error?.type === "AuthExpired";

const getErrorMessage = (error: any) => {
  if (typeof error === "string") return error;
  if (error?.type === "AuthExpired") {
    return (
      error.data?.message ||
      "Your warframe.market session has expired. Please sign in again to continue."
    );
  }
  if (typeof error?.data === "string") return error.data;
  if (error?.data?.message) return error.data.message;
  if (error?.message) return error.message;
  return JSON.stringify(error);
};

function App() {
  const [loading, setLoading] = useState(true);
  const debounceTimeouts = useRef<Record<string, any>>({});
  const authRefreshInProgress = useRef(false);
  const pendingAuthRetry = useRef<(() => Promise<void>) | null>(null);
  const [authenticated, setAuthenticated] = useState(false);
  const [accountName, setAccountName] = useState<string | null>(null);
  const [standings, setStandings] = useState<SyndicateState[]>([]);

  // Auth inputs
  const [authLoading, setAuthLoading] = useState(false);
  const [authError, setAuthError] = useState<string | null>(null);

  // Publish tracking
  const [publishingFactions, setPublishingFactions] = useState<Set<string>>(new Set());

  // Publish preview modal state
  const [publishPreview, setPublishPreview] = useState<{
    factionKey: string;
    quantity: number;
    mods: string[];
  } | null>(null);

  // Sale form & catalog state
  const [allMods, setAllMods] = useState<string[]>([]);
  const [saleItem, setSaleItem] = useState("");
  const [saleQty, setSaleQty] = useState(1);
  const [saleLoading, setSaleLoading] = useState(false);

  // Overlap resolution modal state
  const [overlapData, setOverlapData] = useState<{
    itemSlug: string;
    eligibleFactions: string[];
    quantity: number;
  } | null>(null);

  // Logging
  const [logs, setLogs] = useState<LogEntry[]>([]);

  // Available update from GitHub Releases (null = up to date)
  const [availableUpdate, setAvailableUpdate] = useState<Update | null>(null);
  const [updateInstalling, setUpdateInstalling] = useState(false);

  // Mod catalog preview states
  const [factionMods, setFactionMods] = useState<Record<string, string[]>>({});
  const [expandedFactions, setExpandedFactions] = useState<Set<string>>(new Set());

  const fetchModsForFaction = async (factionKey: string, rank: number) => {
    try {
      const list = await invoke<string[]>("get_faction_mods", {
        factionKey,
        rank,
      });
      setFactionMods((prev) => ({ ...prev, [factionKey]: list }));
    } catch (e) {
      console.error(`Failed to fetch mods for ${factionKey}:`, e);
    }
  };

  const toggleFactionExpand = (factionKey: string) => {
    setExpandedFactions((prev) => {
      const next = new Set(prev);
      if (next.has(factionKey)) {
        next.delete(factionKey);
      } else {
        next.add(factionKey);
      }
      return next;
    });
  };

  const addLog = (message: string, type: "success" | "error" | "info" = "info") => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs((prev) => [{ timestamp, message, type }, ...prev].slice(0, 50));
  };

  const startInAppLogin = async (
    logMessage = "Launching secure in-app verification browser...",
    clearExistingError = true,
  ) => {
    if (authRefreshInProgress.current) return;

    authRefreshInProgress.current = true;
    setAuthLoading(true);
    if (clearExistingError) setAuthError(null);
    addLog(logMessage, "info");

    try {
      await invoke("start_in_app_login");
    } catch (e: any) {
      const errStr = getErrorMessage(e);
      setAuthError(errStr);
      addLog(`Failed to start secure login: ${errStr}`, "error");
      setAuthLoading(false);
      authRefreshInProgress.current = false;
      pendingAuthRetry.current = null;
    }
  };

  const handleAuthExpired = async (error: any, retry?: () => Promise<void>) => {
    pendingAuthRetry.current = retry || null;
    setAuthenticated(false);
    setAccountName(null);
    setAuthError(getErrorMessage(error));
    await startInAppLogin(
      "Warframe Market session expired. Opening secure sign-in to refresh it...",
      false,
    );
  };

  const loadData = async () => {
    try {
      const data: InitData = await invoke("load_syndicates");
      setStandings(data.standings);
      setAuthenticated(data.authenticated);
      setAccountName(data.account_name);

      if (data.auth_refresh_required) {
        void handleAuthExpired({
          type: "AuthExpired",
          data: {
            message:
              "Your saved warframe.market session expired. Please sign in again to continue.",
          },
        });
      }

      // Load mods for all active factions on mount
      for (const s of data.standings) {
        await fetchModsForFaction(s.faction_key, s.rank);
      }

      addLog("Successfully synced standing ledgers with disk data.", "success");
      if (data.authenticated && data.account_name) {
        addLog(`Authenticated with Warframe Market as: ${data.account_name}`, "info");
      }
    } catch (e: any) {
      addLog(`Failed to load system data: ${JSON.stringify(e)}`, "error");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    let unlisten: (() => void) | null = null;

    const checkUpdates = async () => {
      console.log("[Updater] Checking for updates...");
      try {
        const update = await check();
        if (update) {
          console.log(`[Updater] New update available: v${update.version}`);
          addLog(`Auto-update: A new version v${update.version} is available.`, "info");
          setAvailableUpdate(update);
        } else {
          console.log("[Updater] No update available. App is up to date.");
          addLog("Auto-update check: App is up to date.", "info");
          setAvailableUpdate(null);
        }
      } catch (err: any) {
        const errMessage = err?.toString() || JSON.stringify(err);
        console.error("[Updater] Update check failed:", err);
        addLog(`Auto-update check failed: ${errMessage}`, "error");
      }
    };

    const init = async () => {
      await loadData();

      // Load the full mod list from the backend (single source of truth)
      try {
        const mods = await invoke<string[]>("get_all_mods");
        setAllMods(mods);
        if (mods.length > 0) setSaleItem(mods[0]);
      } catch (e) {
        console.error("Failed to load mod catalog:", e);
      }

      // Initial check for updates immediately on load
      await checkUpdates();
    };

    // Register event listener for automated browser authentication before init,
    // because init may automatically open the login window for an expired token.
    void listen<string>("auth_success", async (event) => {
      const retry = pendingAuthRetry.current;
      pendingAuthRetry.current = null;
      authRefreshInProgress.current = false;
      setAuthenticated(true);
      setAccountName(event.payload);
      setAuthLoading(false);
      setAuthError(null);
      addLog(`Authentication successful. Welcome back, Tenno ${event.payload}!`, "success");
      await loadData();
      if (retry) {
        addLog("Session refreshed. Resuming interrupted market action...", "info");
        await retry();
      }
    }).then((fn) => {
      unlisten = fn;
      void init();
    });

    // Run background checks for updates every 15 minutes
    const checkInterval = setInterval(checkUpdates, 15 * 60 * 1000);

    return () => {
      clearInterval(checkInterval);
      if (unlisten) {
        unlisten();
      }
    };
  }, []);

  const handleStartInAppLogin = async () => {
    pendingAuthRetry.current = null;
    await startInAppLogin();
  };

  const handleCancelLogin = async () => {
    try {
      await invoke("cancel_login");
    } catch {
      // Ignore errors — window may have already been closed
    }
    setAuthLoading(false);
    setAuthError(null);
    authRefreshInProgress.current = false;
    pendingAuthRetry.current = null;
    addLog("Login cancelled by user.", "info");
  };

  const handleLogout = async () => {
    try {
      await invoke("logout");
      setAuthenticated(false);
      setAccountName(null);
      addLog("Successfully logged out. Cleared token and cookies.", "success");
    } catch (e: any) {
      addLog(`Failed to log out: ${e.toString()}`, "error");
    }
  };

  const handleStandingChange = (factionKey: string, standing: number) => {
    // Immediate reactive local update
    setStandings((prev) =>
      prev.map((s) => (s.faction_key === factionKey ? { ...s, standing } : s)),
    );

    if (debounceTimeouts.current[factionKey]) {
      clearTimeout(debounceTimeouts.current[factionKey]);
    }

    debounceTimeouts.current[factionKey] = setTimeout(async () => {
      try {
        await invoke("update_standing", { factionKey, standing, rank: null });
      } catch (e: any) {
        addLog(`Sync error for ${factionKey}: ${JSON.stringify(e)}`, "error");
      }
    }, 150);
  };

  const handleRankChange = async (factionKey: string, rank: number) => {
    // Immediate reactive local update
    setStandings((prev) => prev.map((s) => (s.faction_key === factionKey ? { ...s, rank } : s)));
    addLog(`Faction rank updated: ${formatFactionName(factionKey)} to Rank ${rank}.`, "info");

    // Fetch updated list of mods matching the new rank level
    await fetchModsForFaction(factionKey, rank);

    try {
      const updated: SyndicateState = await invoke("update_standing", {
        factionKey,
        standing: standings.find((s) => s.faction_key === factionKey)?.standing || 0,
        rank,
      });
      // Sync corrected standing capacity if adjusted
      setStandings((prev) => prev.map((s) => (s.faction_key === factionKey ? updated : s)));
    } catch (e: any) {
      addLog(`Failed to update rank for ${factionKey}: ${JSON.stringify(e)}`, "error");
    }
  };

  const handlePublish = async (factionKey: string) => {
    setPublishPreview(null);
    setPublishingFactions((prev) => {
      const next = new Set(prev);
      next.add(factionKey);
      return next;
    });
    addLog(`Publishing active sell orders for ${formatFactionName(factionKey)}...`, "info");

    try {
      const mods: string[] = await invoke("publish_syndicate", { factionKey });
      addLog(
        `Successfully published/undercut ${mods.length} listings for ${formatFactionName(factionKey)}!`,
        "success",
      );
    } catch (e: any) {
      if (isAuthExpiredError(e)) {
        await handleAuthExpired(e, () => handlePublish(factionKey));
      } else {
        const errStr = getErrorMessage(e);
        addLog(`Publish failed for ${formatFactionName(factionKey)}: ${errStr}`, "error");
      }
    } finally {
      setPublishingFactions((prev) => {
        const next = new Set(prev);
        next.delete(factionKey);
        return next;
      });
    }
  };

  const handleRecordSale = async (factionChoice: string | null = null) => {
    setSaleLoading(true);
    addLog(`Recording sale: ${saleQty}x ${saleItem}...`, "info");

    try {
      const choice = factionChoice || null;
      const attributed: string = await invoke("record_sale", {
        itemSlug: saleItem,
        quantity: saleQty,
        factionChoice: choice,
      });

      addLog(
        `Sale recorded! Attributed standing deduction to ${formatFactionName(attributed)}. Updated active listings.`,
        "success",
      );
      setOverlapData(null);
      await loadData();
    } catch (e: any) {
      if (e.type === "Overlap") {
        addLog(
          `Attribution conflict: '${saleItem}' belongs to multiple represented syndicates.`,
          "info",
        );
        setOverlapData({
          itemSlug: e.data.item_slug,
          eligibleFactions: e.data.eligible_factions,
          quantity: saleQty,
        });
      } else if (e.type === "InsufficientStanding") {
        addLog(`Insufficient standing: Cannot deduct standing for '${saleItem}'.`, "error");
      } else if (isAuthExpiredError(e)) {
        await handleAuthExpired(e);
        addLog("After signing in, please retry logging the sale.", "info");
      } else {
        const errStr = getErrorMessage(e);
        addLog(`Failed to log sale: ${errStr}`, "error");
      }
    } finally {
      setSaleLoading(false);
    }
  };

  const handleInstallUpdate = async () => {
    if (!availableUpdate) return;
    setUpdateInstalling(true);
    addLog(`Downloading update v${availableUpdate.version}...`, "info");
    try {
      await availableUpdate.downloadAndInstall();
      addLog("Update installed successfully. Relaunching...", "success");
      await relaunch();
    } catch (e: any) {
      addLog(`Update failed: ${e?.toString()}`, "error");
      setUpdateInstalling(false);
    }
  };

  const getMaxStanding = (rank: number) => {
    switch (rank) {
      case 5:
        return 132000;
      case 4:
        return 99000;
      case 3:
        return 70000;
      case 2:
        return 44000;
      case 1:
        return 22000;
      default:
        return 5000;
    }
  };

  const formatFactionName = (key: string) => {
    return key.replace(/_/g, " ");
  };

  if (loading) {
    return (
      <div className="auth-wrapper">
        <div className="auth-card">
          <svg className="auth-icon spinner" viewBox="0 0 24 24">
            <path
              d="M12 4V2m0 20v-2m8-8h2M2 12h2m13.657-5.657l1.414-1.414M5.636 18.364l1.414-1.414m0-11.314L5.636 5.636m12.728 12.728l-1.414-1.414"
              stroke="#c79b3d"
              strokeWidth="2"
              strokeLinecap="round"
            />
          </svg>
          <h2>Initialising Ledgers</h2>
          <p>Handshaking with safe Rust memory channels...</p>
        </div>
      </div>
    );
  }

  if (!authenticated) {
    return (
      <div className="auth-wrapper">
        <div className="auth-card">
          <svg className="auth-icon" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
            <path d="M18 8h-1V6c0-2.76-2.24-5-5-5S7 3.24 7 6v2H6c-1.1 0-2 .9-2 2v10c0 1.1.9 2 2 2h12c1.1 0 2-.9 2-2V10c0-1.1-.9-2-2-2zm-6 9c-1.1 0-2-.9-2-2s.9-2 2-2 2 .9 2 2-.9 2-2 2zm3.1-9H8.9V6c0-1.71 1.39-3.1 3.1-3.1 1.71 0 3.1 1.39 3.1 3.1v2z" />
          </svg>
          <h2>Tenno Verification</h2>
          <p>
            Authenticate safely with your <code>warframe.market</code> account. Logging in via our
            secure In-App Browser supports Steam, Discord, Xbox, PSN, and Email.
          </p>

          <div
            style={{
              display: "flex",
              flexDirection: "column",
              gap: "16px",
              marginTop: "8px",
            }}
          >
            <button
              type="button"
              className="btn-primary"
              onClick={handleStartInAppLogin}
              disabled={authLoading}
            >
              {authLoading ? "Waiting for Authentication..." : "Secure Sign In"}
            </button>

            {authLoading && (
              <>
                <p
                  style={{
                    fontSize: "0.85rem",
                    fontStyle: "italic",
                    opacity: 0.8,
                  }}
                >
                  Please complete the login in the pop-up window. Once logged in, it will close
                  automatically.
                </p>
                <button
                  type="button"
                  className="btn-secondary"
                  onClick={handleCancelLogin}
                  style={{ fontSize: "0.8rem" }}
                >
                  Cancel
                </button>
              </>
            )}
          </div>

          {authError && <div className="auth-error">{authError}</div>}
        </div>
      </div>
    );
  }

  return (
    <div className="app-container">
      {/* Update Available Premium Banner */}
      {availableUpdate && (
        <div className="update-banner">
          <div className="update-banner-content">
            <svg
              className="update-banner-icon"
              viewBox="0 0 24 24"
              width="20"
              height="20"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
              <polyline points="7 10 12 15 17 10" />
              <line x1="12" y1="15" x2="12" y2="3" />
            </svg>
            <span className="update-banner-text">
              New version <strong>v{availableUpdate.version}</strong> is available.
            </span>
          </div>
          <button
            className="btn-update-install"
            disabled={updateInstalling}
            onClick={handleInstallUpdate}
          >
            {updateInstalling ? "Installing..." : "Install & Restart"}
          </button>
        </div>
      )}

      {/* Header bar */}
      <header className="app-header">
        <div className="header-title-group">
          <svg className="header-logo" viewBox="0 0 24 24">
            <path d="M12 2L2 22h20L12 2zm0 4l6 12H6l6-12z" />
          </svg>
          <h1 className="app-title">Tenno Syndicate Automator</h1>
        </div>

        <div className="user-badge-container">
          <div className="user-badge">
            <span className="badge-dot"></span>
            <span>{accountName || "Authenticated Tenno"}</span>
          </div>
          <button type="button" className="btn-logout" onClick={handleLogout}>
            Logout
          </button>
        </div>
      </header>

      {/* Dashboard Factions Grid */}
      <section className="dashboard-grid">
        {standings.map((state) => {
          const maxCap = getMaxStanding(state.rank);
          const isPublishing = publishingFactions.has(state.faction_key);
          const listQty = Math.floor(state.standing / 25000);

          return (
            <div
              key={state.faction_key}
              className="syndicate-card"
              style={
                {
                  "--faction-glow": `var(--color-${state.faction_key})`,
                } as React.CSSProperties
              }
            >
              {/* Card Header */}
              <div className="card-header">
                <div className="faction-title-group">
                  <h3 className="faction-name">{formatFactionName(state.faction_key)}</h3>
                  <span className="faction-max-info">CAPACITY: {maxCap.toLocaleString()}</span>
                </div>

                <div className="rank-badge-group">
                  <span className="rank-label">Rank</span>
                  <select
                    className="rank-select"
                    value={state.rank}
                    onChange={(e) => handleRankChange(state.faction_key, parseInt(e.target.value))}
                  >
                    {[0, 1, 2, 3, 4, 5].map((r) => (
                      <option key={r} value={r}>
                        {r}
                      </option>
                    ))}
                  </select>
                </div>
              </div>

              {/* Slider Controller */}
              <div className="slider-container">
                <div className="slider-labels">
                  <span style={{ color: "var(--text-secondary)" }}>STANDING</span>
                  <span className="standing-value">
                    {state.standing.toLocaleString()} / {maxCap.toLocaleString()}
                  </span>
                </div>
                <input
                  type="range"
                  className="standing-slider"
                  min="0"
                  max={maxCap}
                  value={state.standing}
                  onChange={(e) =>
                    handleStandingChange(state.faction_key, parseInt(e.target.value))
                  }
                />
              </div>

              {/* Listable count info */}
              <div className="offerings-info">
                <span className="offerings-label">LISTABLE OFFERINGS</span>
                <span className="offerings-count">{listQty > 0 ? listQty : 0}</span>
              </div>

              {/* Mod catalog preview */}
              {(() => {
                const modsList = factionMods[state.faction_key] || [];
                const isExpanded = expandedFactions.has(state.faction_key);
                return (
                  <div className="mods-preview-container">
                    <button
                      type="button"
                      className="btn-mods-toggle"
                      onClick={() => toggleFactionExpand(state.faction_key)}
                    >
                      {isExpanded
                        ? "Hide Mod Catalog"
                        : `View Mod Catalog (${modsList.length} items)`}
                    </button>
                    {isExpanded && (
                      <div className="mods-list-drawer">
                        {modsList.length === 0 ? (
                          <span className="no-mods-msg">No mods available at this rank level.</span>
                        ) : (
                          modsList.map((m) => (
                            <span key={m} className="mod-pill">
                              {m.replace(/_/g, " ")}
                            </span>
                          ))
                        )}
                      </div>
                    )}
                  </div>
                );
              })()}

              {/* Faction Actions */}
              <div className="syndicate-actions">
                <button
                  className={`btn-action ${isPublishing ? "publishing" : ""}`}
                  disabled={isPublishing}
                  onClick={() => {
                    setPublishPreview({
                      factionKey: state.faction_key,
                      quantity: Math.floor(state.standing / 25000),
                      mods: factionMods[state.faction_key] || [],
                    });
                  }}
                >
                  {isPublishing ? (
                    <>
                      <svg
                        className="spinner"
                        viewBox="0 0 24 24"
                        style={{ width: 14, height: 14 }}
                      >
                        <path
                          d="M12 4V2m0 20v-2m8-8h2M2 12h2"
                          stroke="currentColor"
                          strokeWidth="2"
                        />
                      </svg>
                      Updating...
                    </>
                  ) : (
                    "Publish Offerings"
                  )}
                </button>
              </div>
            </div>
          );
        })}
      </section>

      {/* Sales Record Quick Form */}
      <section className="ledger-panel">
        <h2 className="panel-title">Record Market Transaction</h2>

        <div className="ledger-form">
          <div className="form-group" style={{ flex: 2 }}>
            <label htmlFor="mod-slug">Offering Item Mod</label>
            <select
              id="mod-slug"
              className="form-select"
              value={saleItem}
              onChange={(e) => setSaleItem(e.target.value)}
            >
              {allMods.map((m) => (
                <option key={m} value={m}>
                  {m.replace(/_/g, " ")}
                </option>
              ))}
            </select>
          </div>

          <div className="form-group">
            <label htmlFor="quantity">Sale Quantity</label>
            <input
              id="quantity"
              type="number"
              className="form-number-input"
              min="1"
              max="10"
              value={saleQty}
              onChange={(e) => setSaleQty(Math.max(1, parseInt(e.target.value) || 1))}
            />
          </div>

          <button
            className="btn-record"
            disabled={saleLoading}
            onClick={() => handleRecordSale(null)}
          >
            {saleLoading ? "Logging..." : "Log Completed Sale"}
          </button>
        </div>
      </section>

      {/* Logging Ticker Console */}
      <footer className="log-console">
        <div className="console-header">Transaction Ticker Console Logs</div>
        <div className="console-logs">
          {logs.length === 0 ? (
            <div className="log-entry" style={{ color: "#475569" }}>
              <span>[SYSTEM LOGS]</span>
              <span>No transactions executed in this session. Awaiting operations...</span>
            </div>
          ) : (
            logs.map((log, idx) => (
              <div key={idx} className="log-entry">
                <span className="log-timestamp">[{log.timestamp}]</span>
                <span className={`log-msg ${log.type}`}>{log.message}</span>
              </div>
            ))
          )}
        </div>
      </footer>

      {/* Frosted Confirm Publish Modal */}
      {publishPreview && (
        <div className="modal-overlay">
          <div
            className="modal-content publish-confirm"
            style={
              {
                "--faction-glow": `var(--color-${publishPreview.factionKey})`,
              } as React.CSSProperties
            }
          >
            <div className="modal-header">
              <h3>Confirm Market Publication</h3>
            </div>

            <div className="modal-body">
              <p>
                You are about to publish active sell orders for{" "}
                <strong>{formatFactionName(publishPreview.factionKey)}</strong>.
              </p>

              {publishPreview.quantity > 0 ? (
                <div className="publish-preview-info">
                  <div className="publish-stat-box">
                    <span className="stat-label">Quantity per Mod</span>
                    <span className="stat-value">{publishPreview.quantity}</span>
                  </div>
                  <div className="publish-stat-box">
                    <span className="stat-label">Total Mods to Publish</span>
                    <span className="stat-value">{publishPreview.mods.length}</span>
                  </div>
                </div>
              ) : (
                <div className="publish-preview-info warning">
                  <p className="warning-text">
                    Your standing is{" "}
                    <strong>
                      {standings
                        .find((s) => s.faction_key === publishPreview.factionKey)
                        ?.standing.toLocaleString()}
                    </strong>
                    , which is below the 25,000 threshold.
                  </p>
                </div>
              )}

              {publishPreview.quantity > 0 ? (
                <p style={{ marginTop: "16px", marginBottom: "8px" }}>
                  The following <strong>{publishPreview.mods.length}</strong> mods will be posted or
                  updated at the lowest active market price (undercut by 1 Platinum):
                </p>
              ) : (
                <p
                  style={{
                    marginTop: "16px",
                    marginBottom: "8px",
                    color: "var(--color-danger)",
                  }}
                >
                  All existing active listings for these{" "}
                  <strong>{publishPreview.mods.length}</strong> mods will be{" "}
                  <strong>DELETED</strong> from your profile:
                </p>
              )}

              <div className="preview-mods-list">
                {publishPreview.mods.length === 0 ? (
                  <span className="no-mods-msg">No mods available at this rank level.</span>
                ) : (
                  publishPreview.mods.map((m) => (
                    <span key={m} className="mod-pill-preview">
                      {m.replace(/_/g, " ")}
                    </span>
                  ))
                )}
              </div>

              {publishPreview.quantity > 0 && (
                <div className="preview-disclaimer">
                  <svg viewBox="0 0 24 24" width="16" height="16" fill="var(--color-gold)">
                    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-6h2v6zm0-8h-2V7h2v2z" />
                  </svg>
                  <span>
                    Prices are queried live from warframe.market. Standing is only deducted when you
                    record a sale.
                  </span>
                </div>
              )}
            </div>

            <div className="modal-footer" style={{ gap: "12px" }}>
              <button className="btn-secondary" onClick={() => setPublishPreview(null)}>
                Cancel
              </button>
              <button
                className="btn-action btn-confirm-publish"
                style={{
                  background:
                    publishPreview.quantity > 0
                      ? "linear-gradient(135deg, var(--faction-glow) 0%, rgba(0,0,0,0.6) 100%)"
                      : "linear-gradient(135deg, var(--color-danger) 0%, rgba(0,0,0,0.6) 100%)",
                  boxShadow: `0 0 10px var(--faction-glow)`,
                  color: "#fff",
                  border: "1px solid var(--faction-glow)",
                  padding: "10px 20px",
                  borderRadius: "8px",
                  cursor: "pointer",
                  fontWeight: "bold",
                  textTransform: "uppercase",
                  fontSize: "0.85rem",
                  letterSpacing: "1px",
                  transition: "all 0.25s ease",
                }}
                onClick={() => handlePublish(publishPreview.factionKey)}
              >
                {publishPreview.quantity > 0 ? "Confirm & Publish" : "Confirm Deletion"}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Frosted Attribution Overlap Modal */}
      {overlapData && (
        <div className="modal-overlay">
          <div className="modal-content">
            <div className="modal-header">
              <h3>Deduction Attribution Conflict</h3>
            </div>

            <div className="modal-body">
              <p>
                The offering item <strong>{overlapData.itemSlug.replace(/_/g, " ")}</strong> is
                shared across multiple represented factions that meet the standing requirement.
              </p>
              <p style={{ marginTop: "8px" }}>
                Select which Syndicate's standing should be deducted for this transaction:
              </p>

              <div className="overlap-grid">
                {overlapData.eligibleFactions.map((f) => (
                  <button
                    key={f}
                    className="btn-faction-choice"
                    style={
                      {
                        "--faction-glow": `var(--color-${f})`,
                      } as React.CSSProperties
                    }
                    onClick={() => handleRecordSale(f)}
                  >
                    <span>{formatFactionName(f)}</span>
                  </button>
                ))}
              </div>
            </div>

            <div className="modal-footer">
              <button className="btn-secondary" onClick={() => setOverlapData(null)}>
                Cancel Transaction
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
