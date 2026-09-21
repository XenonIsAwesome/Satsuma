import { useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { basename } from "../lib/path";
import type { DroppedFile, FileCategory, MenuMode } from "../types";

interface RawLaunchRequest {
  mode: MenuMode;
  paths: string[];
  /** Computed Rust-side (a plain extension check, no I/O) so the overlay
   * never has to wait on an async round-trip before it knows what to
   * render — see the doc comment on `LaunchRequest::category` in
   * `satsuma-core`. Resolving this asynchronously used to leave a window,
   * however brief, where the overlay was already shown and focused but had
   * nothing to render yet, which could catch clicks meant for whatever was
   * behind it. */
  category: FileCategory;
}

/**
 * Handles a `LaunchRequest` delivered from outside the normal drag-and-drop
 * flow: a Linux file-manager "Convert with Satsuma" / "Satsuma Tools"
 * context-menu entry launches the app with `--mode=<mode> <path>...`, and
 * (on an already-running instance) the single-instance plugin forwards that
 * as a `launch-request` event instead. Either way, the app should open the
 * wedge menu directly for the given file(s) and mode, skipping the
 * Shift/Alt step since the context-menu click was itself the deliberate
 * trigger.
 */
export function useLaunchRequest(onLaunchRequest: (file: DroppedFile, mode: MenuMode) => void) {
  const resolve = useCallback(
    (request: RawLaunchRequest | null) => {
      if (!request) return;
      const path = request.paths[0];
      if (!path) return;
      onLaunchRequest({ path, name: basename(path), category: request.category }, request.mode);
    },
    [onLaunchRequest],
  );

  useEffect(() => {
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
      return;
    }

    void invoke<RawLaunchRequest | null>("take_launch_request").then(resolve);

    let unlisten: (() => void) | undefined;
    let cancelled = false;

    listen<RawLaunchRequest>("launch-request", (event) => {
      void resolve(event.payload);
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [resolve]);
}
