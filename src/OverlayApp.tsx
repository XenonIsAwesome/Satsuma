import { useCallback, useEffect, useRef, useState } from "react";
import { WedgeMenu } from "./components/WedgeMenu";
import { useLaunchRequest } from "./hooks/useLaunchRequest";
import { useHeldModifiers } from "./hooks/useHeldModifiers";
import { useSettings } from "./hooks/useSettings";
import { formatWedgeOptions, getToolOptions } from "./data/wedgeOptions";
import { comboContains, comboLabel, extraToolsKeys } from "./lib/modifiers";
import { extensionOf } from "./lib/path";
import { convertFile, extractArchive, hideOverlay, listConversionTargets, showOverlayWindow } from "./lib/tauri";
import type { DroppedFile, MenuMode, WedgeOption } from "./types";
import "./OverlayApp.css";

/** Renders just the wedge menu, transparent-background, for the borderless
 * overlay window that a Windows Shift-hotkey press or a Linux file-manager
 * context-menu click opens — as opposed to `App`, which is the normal
 * titled window used only for drag-and-drop. The window itself is
 * positioned and shown/hidden by the Rust side (`show_overlay`/
 * `hide_overlay` in lib.rs); this component only reacts to the
 * `LaunchRequest` that accompanies each trigger. */
function OverlayApp() {
  const [file, setFile] = useState<DroppedFile | null>(null);
  const [mode, setMode] = useState<MenuMode>("formats");
  const [open, setOpen] = useState(false);
  // A conversion/extraction failure keeps the overlay open and surfaces the
  // error inline (see `handleSelect`) — the overlay's equivalent of the
  // drag-and-drop toasts in App.tsx.
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  // True while a convertFile/extractArchive invoke is in flight. Gates
  // `handleSelect`: a second wedge click mid-conversion would otherwise
  // fire a second, concurrent invoke racing the first toward the same
  // sibling path (image-family saves only materialize the file at
  // completion, so both would resolve the same name and tear the output).
  const [converting, setConverting] = useState(false);
  // Informational text for the in-flight banner below (e.g. "Converting to
  // JPG…"); lives next to `converting` and is cleared wherever that flag is
  // cleared, so the indicator is always gone by the time the session-gated
  // `.then`/`.catch` continuation runs.
  const [convertingHint, setConvertingHint] = useState<string | null>(null);
  // Counts menu sessions — incremented every time a launch request opens
  // the overlay. A conversion/extraction completion is only allowed to
  // close the overlay / surface an error when the session that started it
  // is still the one on screen: Escape-then-re-trigger (or a second
  // trigger) while the work still runs would otherwise let a stale
  // completion's close() hide an overlay the user has since re-opened —
  // the same stale-async-continuation hide race as the Rust-side
  // single-instance fix, now on the frontend side.
  const sessionRef = useRef(0);
  // Backend-queried as of Phase 2 (list_conversion_targets), replacing
  // Phase 0's hardcoded per-category format table.
  const [formatOptions, setFormatOptions] = useState<WedgeOption[]>([]);
  const { settings } = useSettings();
  // Whichever keys the *original* trigger needed may already have been
  // held before this window had OS focus, and so never produced an
  // observable `keydown` here — only the keys tools needs *beyond* format
  // (Alt, for the default Shift/Shift+Alt bindings, plus tools' own extra
  // key if it's configured with one) are ever reliably observable while
  // the overlay is open. See `extraToolsKeys`.
  const extraTools = extraToolsKeys(settings.formatModifiers, settings.toolsModifiers);
  // Only watches that specific extra key, not `lib/keys.ts`'s whole
  // supported set — see `useHeldModifiers`'s own doc comment for why: this
  // window's only content is the wedge menu itself, whose own arrow-key
  // navigation must not be mistaken for a hotkey and trigger a spurious
  // re-render/options-reset.
  const held = useHeldModifiers(extraTools.key ? [extraTools.key] : []);
  const toolsMatched = comboContains(held, extraTools);
  // The launch request's own mode (e.g. from a Linux "Satsuma Tools" vs.
  // "Convert with Satsuma" context-menu entry, or the held combination
  // sampled at the moment a Windows hotkey trigger fired) is authoritative
  // right up until the menu is open — skip the first tools-combo-driven
  // sync so a click that opened straight into tools mode isn't immediately
  // flipped back to formats just because the tools combo wasn't literally
  // held for that click. Once the user actually presses or releases it
  // while the menu is visible, live-toggle it from then on, matching the
  // main window's behavior.
  const hasSyncedOnce = useRef(false);
  // The pending id of the deferred `showOverlayWindow()` call scheduled by
  // `handleLaunchRequest` below (see its own comment), so `close` can cancel
  // it — otherwise a stale timeout firing after the user has already
  // dismissed the overlay (Escape, with no new launch request following)
  // would silently re-show a window the user just closed. A new launch
  // request replaces this ref's entry outright, so only ever one deferred
  // show is pending at a time.
  const showTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleLaunchRequest = useCallback((droppedFile: DroppedFile, requestMode: MenuMode) => {
    // A new session: whatever in-flight work a *previous* session may still
    // have running must neither gate nor be allowed to close this one.
    sessionRef.current += 1;
    setConverting(false);
    setConvertingHint(null);
    setFile(droppedFile);
    setMode(requestMode);
    setErrorMessage(null);
    setOpen(true);
    // Re-assert overlay visibility from the page once a LaunchRequest has
    // actually opened the wedge menu. The Rust-side show that normally
    // accompanies each trigger (`show_overlay` in lib.rs) is unreliable on
    // Windows for a *forwarded* request: the single-instance plugin
    // delivers it inside a WM_COPYDATA wndproc on the main thread, where
    // WebView2's first-show can silently no-op and leave the overlay HWND
    // existing but hidden (seen on e2e-windows CI). An `invoke` rides the
    // normal IPC path instead, which is only processed once the wndproc
    // has returned — a dependable spot to re-assert "menu open ⇒ window
    // visible". Idempotent for a direct launch (the setup-time show has
    // already positioned/revealed the window by the time the page mounts).
    // Skipped for an unsupported file (category "unknown"): the Rust side
    // deliberately refuses to open the overlay for those (nothing to
    // render), and re-asserting visibility here would undo that.
    if (droppedFile.category !== "unknown") {
      // On Linux (WebKitGTK) the overlay can paint transparent on the very
      // first show right after a context-menu launch — delay the show until
      // the WebView has committed and painted a frame, eliminating the need
      // for the user to right-click → Refresh. setTimeout (not rAF) is used
      // because jsdom doesn't implement requestAnimationFrame. Any
      // previously-scheduled show is superseded by this one (see
      // `showTimeoutRef`'s doc comment); `close` cancels it outright if the
      // user dismisses the overlay before it fires.
      if (showTimeoutRef.current !== null) {
        clearTimeout(showTimeoutRef.current);
      }
      showTimeoutRef.current = setTimeout(() => {
        showTimeoutRef.current = null;
        void showOverlayWindow();
      }, 0);
    }
  }, []);
  useLaunchRequest(handleLaunchRequest);

  useEffect(() => {
    if (!file) {
      setFormatOptions([]);
      return;
    }
    let cancelled = false;
    listConversionTargets([file.path])
      .then((targets) => {
        if (!cancelled) setFormatOptions(formatWedgeOptions(targets));
      })
      .catch(() => {
        if (!cancelled) setFormatOptions([]);
      });
    return () => {
      cancelled = true;
    };
  }, [file]);

  useEffect(() => {
    if (!open) {
      hasSyncedOnce.current = false;
      return;
    }
    if (!hasSyncedOnce.current) {
      hasSyncedOnce.current = true;
      return;
    }
    setMode(toolsMatched ? "tools" : "formats");
  }, [toolsMatched, open]);

  const close = useCallback(() => {
    setOpen(false);
    // A hidden overlay must never stay gated by an in-flight conversion the
    // user has already dismissed (the next trigger clears it anyway, but
    // this keeps "closed ⇒ never gated" true even mid-flight).
    setConverting(false);
    setConvertingHint(null);
    // Cancel a still-pending deferred show (see `showTimeoutRef`'s doc
    // comment) so it can't fire after this dismissal and silently re-show
    // the window the user just closed.
    if (showTimeoutRef.current !== null) {
      clearTimeout(showTimeoutRef.current);
      showTimeoutRef.current = null;
    }
    void hideOverlay();
  }, []);

  // The overlay opens at the OS cursor without any prior click, so on
  // Windows its WebView2 control may never have received input focus —
  // an Escape/arrow key injected from outside then goes to whichever
  // window was focused before instead of this page. Request focus
  // explicitly once the menu is open, so the very first overlay is
  // keyboard-reachable without a click (the e2e-windows escape-cancel
  // scenario depends on this); harmless on Linux, where window activation
  // is handled Rust-side.
  useEffect(() => {
    if (open) window.focus();
  }, [open]);

  // WedgeMenu only listens for Escape once it has options to render, so on
  // its own it can't dismiss an overlay that's open but showing nothing
  // (e.g. wedgeOptions ending up empty for some reason). Listen here too,
  // independent of what WedgeMenu renders, so Escape always closes the
  // overlay window while it's open — belt-and-suspenders against ever
  // being stuck with an invisible, click-catching window.
  useEffect(() => {
    if (!open) return;
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") close();
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, close]);

  const handleSelect = useCallback(
    (option: WedgeOption) => {
      // One conversion/extraction at a time (see the `converting` doc
      // comment): while one is in flight, ignore every wedge click rather
      // than launching a second concurrent invoke racing it for the same
      // output path. Dismissing stays available — the backdrop/Escape
      // paths call `close`, not this.
      if (converting) return;
      const session = sessionRef.current;
      if (mode === "formats" && file) {
        // Phase 2: a real conversion runs. Success closes the overlay
        // immediately (matching how a real Tangerine-style trigger
        // dismisses on selection); failure keeps it open and surfaces the
        // error inline instead — a broken conversion (no FFmpeg vendored,
        // disk error, ...) must never be silently swallowed by a closing
        // overlay with no indication anything happened. Same message shape
        // as the drag-and-drop path in App.tsx. Both continuations are
        // gated on their session: a completion for a session that was
        // Escaped/re-triggered since must neither close the new session's
        // overlay nor touch its state (see `sessionRef`'s doc comment).
        setConverting(true);
        setConvertingHint(`Converting to ${option.label}…`);
        convertFile(file.path, option.id)
          .then(() => {
            if (sessionRef.current !== session) return;
            setConverting(false);
            setConvertingHint(null);
            close();
          })
          .catch((error: unknown) => {
            if (sessionRef.current !== session) return;
            setConverting(false);
            setConvertingHint(null);
            console.error("[satsuma] conversion failed", error);
            setErrorMessage(`Convert to ${option.label} failed for ${file.name}: ${String(error)}`);
          });
      } else if (option.id === "extract" && file) {
        // Extract Archive is a real, single action (no settings screen),
        // per Tools.md — unlike every other tool wedge, which stays fake
        // until its own dedicated GUI lands in Phase 3+.
        setConverting(true);
        setConvertingHint("Extracting…");
        extractArchive(file.path)
          .then(() => {
            if (sessionRef.current !== session) return;
            setConverting(false);
            setConvertingHint(null);
            close();
          })
          .catch((error: unknown) => {
            if (sessionRef.current !== session) return;
            setConverting(false);
            setConvertingHint(null);
            console.error("[satsuma] extract failed", error);
            setErrorMessage(`Extract failed for ${file.name}: ${String(error)}`);
          });
      } else {
        // Advanced-tool wedges stay fake until their own dedicated GUI
        // lands in Phase 3+: no file is written.
        console.log("[satsuma] selected tool", { option, file });
        close();
      }
    },
    [converting, mode, file, close],
  );

  const wedgeOptions = mode === "formats" ? formatOptions : getToolOptions(file?.category ?? "unknown");
  const hubLabel = file ? (extensionOf(file.path) ?? file.category).toUpperCase() : undefined;

  return (
    <>
      <WedgeMenu
        open={open}
        mode={mode}
        options={wedgeOptions}
        onSelect={handleSelect}
        onCancel={close}
        hubLabel={hubLabel}
        toolsHint={comboLabel(settings.toolsModifiers)}
        unimplementedTools={
          // Join tool IDs that are currently in Phase 0 fake mode, excluding the real "extract" action.
          mode === "tools" && file
            ? ["compress", "crop", "trim", "split", "merge"].filter((id) => getToolOptions(file.category).some((o) => o.id === id)).join(", ") || undefined
            : undefined
        }
      />
      {converting && !errorMessage && convertingHint && (
        <p className="overlay-feedback overlay-feedback--info" role="status" data-testid="converting-feedback">
          {convertingHint}
        </p>
      )}
      {errorMessage && (
        <p className="overlay-feedback overlay-feedback--danger" role="alert" data-testid="overlay-feedback">
          {errorMessage}
        </p>
      )}
    </>
  );
}

export default OverlayApp;
