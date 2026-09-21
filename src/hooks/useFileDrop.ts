import { useCallback, useEffect, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { detectFileCategory } from "../lib/tauri";
import { basename } from "../lib/path";
import type { DroppedFile } from "../types";

/**
 * Subscribes to Tauri's native OS drag-and-drop events for the current
 * webview window (rather than HTML5 drag events) so real file paths are
 * available on both Windows and Linux. No-ops outside a Tauri runtime (e.g.
 * a plain browser or a test environment that hasn't mocked the Tauri API).
 */
export function useFileDrop(onFileDropped: (file: DroppedFile) => void) {
  const [isDragOver, setIsDragOver] = useState(false);

  const handleDroppedPaths = useCallback(
    async (paths: string[]) => {
      const path = paths[0];
      if (!path) return;
      const category = await detectFileCategory(path);
      onFileDropped({ path, name: basename(path), category });
    },
    [onFileDropped],
  );

  useEffect(() => {
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
      return;
    }

    let unlisten: (() => void) | undefined;
    let cancelled = false;

    getCurrentWebviewWindow()
      .onDragDropEvent((event) => {
        switch (event.payload.type) {
          case "enter":
          case "over":
            setIsDragOver(true);
            break;
          case "drop":
            setIsDragOver(false);
            void handleDroppedPaths(event.payload.paths);
            break;
          case "leave":
            setIsDragOver(false);
            break;
        }
      })
      .then((fn) => {
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
  }, [handleDroppedPaths]);

  return { isDragOver };
}
