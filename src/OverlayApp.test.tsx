import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import OverlayApp from "./OverlayApp";
import { DEFAULT_FORMAT_MODIFIERS } from "./lib/modifiers";
import type { Settings } from "./types";

const { invokeMock, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

describe("OverlayApp", () => {
  beforeEach(() => {
    // OverlayApp requests window focus when the overlay opens; jsdom warns
    // "Not implemented" for a bare window.focus() call, so stub it quietly
    // (the dedicated focus test below asserts on its own spy).
    vi.spyOn(window, "focus").mockImplementation(() => {});
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    listenMock.mockResolvedValue(() => {});
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve(null);
        case "hide_overlay":
          return Promise.resolve(undefined);
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        default:
          return Promise.resolve(undefined);
      }
    });
  });

  afterEach(() => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    vi.clearAllMocks();
    vi.restoreAllMocks();
  });

  it("renders nothing until a launch request arrives", () => {
    render(<OverlayApp />);
    expect(screen.queryByTestId("wedge-menu")).not.toBeInTheDocument();
  });

  it("opens the menu directly from the startup launch request, skipping Shift", async () => {
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "tools", paths: ["/home/user/clip.mp4"], category: "video" });
        case "list_conversion_targets":
          return Promise.resolve([]);
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);

    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());
    expect(screen.getByRole("menu")).toHaveAccessibleName("Advanced tools");
  });

  it("uses the category carried on the launch request directly, without an extra detect_file_category call", async () => {
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/photo.png"], category: "image" });
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);

    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());
    expect(invokeMock).not.toHaveBeenCalledWith("detect_file_category", expect.anything());
  });

  it("opens the menu from a launch-request event forwarded by the single-instance plugin", async () => {
    let emitLaunchRequest: ((event: { payload: unknown }) => void) | undefined;
    listenMock.mockImplementation((_name: string, handler: (event: { payload: unknown }) => void) => {
      emitLaunchRequest = handler;
      return Promise.resolve(() => {});
    });

    render(<OverlayApp />);
    await waitFor(() => expect(emitLaunchRequest).toBeDefined());

    act(() => {
      emitLaunchRequest?.({ payload: { mode: "formats", paths: ["/home/user/photo.png"], category: "image" } });
    });

    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());
    expect(screen.getByRole("menu")).toHaveAccessibleName("Convert to format");
  });

  it("re-asserts overlay visibility via show_overlay_window when the startup launch request opens the menu", async () => {
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/photo.png"], category: "image" });
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);

    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());
    // The menu opening must ride the normal IPC path to re-assert window
    // visibility (the Windows single-instance fix — see OverlayApp).
    expect(invokeMock).toHaveBeenCalledWith("show_overlay_window");
  });

  it("re-asserts overlay visibility via show_overlay_window for a forwarded single-instance launch request", async () => {
    let emitLaunchRequest: ((event: { payload: unknown }) => void) | undefined;
    listenMock.mockImplementation((_name: string, handler: (event: { payload: unknown }) => void) => {
      emitLaunchRequest = handler;
      return Promise.resolve(() => {});
    });
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve(null);
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    await waitFor(() => expect(emitLaunchRequest).toBeDefined());

    act(() => {
      emitLaunchRequest?.({ payload: { mode: "formats", paths: ["/home/user/photo.png"], category: "image" } });
    });

    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());
    expect(invokeMock).toHaveBeenCalledWith("show_overlay_window");
  });

  it("cancels the deferred show_overlay_window re-assertion if Escape closes the overlay first", async () => {
    let emitLaunchRequest: ((event: { payload: unknown }) => void) | undefined;
    listenMock.mockImplementation((_name: string, handler: (event: { payload: unknown }) => void) => {
      emitLaunchRequest = handler;
      return Promise.resolve(() => {});
    });
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve(null);
        case "hide_overlay":
          return Promise.resolve(undefined);
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    await waitFor(() => expect(emitLaunchRequest).toBeDefined());

    // Opens the overlay and (synchronously, within this act()) schedules
    // the deferred show_overlay_window re-assertion via setTimeout(..., 0).
    act(() => {
      emitLaunchRequest?.({ payload: { mode: "formats", paths: ["/home/user/photo.png"], category: "image" } });
    });

    // Dismiss synchronously, in the same tick — before that setTimeout(0)
    // macrotask has had a chance to run. The overlay-level Escape listener
    // only depends on `open` (already true), not on the wedge options
    // having loaded, so it's already attached at this point.
    fireEvent.keyDown(window, { key: "Escape" });

    // Give the (real) event loop a turn so the pending timeout would have
    // fired by now if it hadn't been cancelled by close().
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });

    expect(invokeMock).toHaveBeenCalledWith("hide_overlay");
    expect(invokeMock).not.toHaveBeenCalledWith("show_overlay_window");
  });

  it("does not re-assert overlay visibility for an unsupported (unknown) launch request", async () => {
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/report.pptx"], category: "unknown" });
        case "list_conversion_targets":
          return Promise.resolve([]);
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("take_launch_request"));
    // The Rust side deliberately refuses to open the overlay for an
    // unsupported file — the page-side re-assertion must not undo that.
    expect(invokeMock).not.toHaveBeenCalledWith("show_overlay_window");
    expect(screen.queryByTestId("wedge-menu")).not.toBeInTheDocument();
  });

  it("hides the overlay window after a selection is applied", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/clip.mp4"], category: "video" });
        case "hide_overlay":
          return Promise.resolve(undefined);
        case "list_conversion_targets":
          return Promise.resolve(["mov", "mkv", "webm", "avi", "wmv", "gif", "mp3"]);
        case "convert_file":
          return Promise.resolve("/home/user/clip.mov");
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());
    await waitFor(() => expect(screen.getByTestId("wedge-mov")).toBeInTheDocument());

    await user.keyboard("{Enter}");

    await waitFor(() => expect(screen.queryByTestId("wedge-menu")).not.toBeInTheDocument());
    expect(invokeMock).toHaveBeenCalledWith("hide_overlay");
    // A format selection fires the real conversion without waiting on it
    // before closing.
    expect(invokeMock).toHaveBeenCalledWith("convert_file", {
      path: "/home/user/clip.mp4",
      targetExtension: "mov",
    });
  });

  it("does not double-fire a conversion while one is already in flight", async () => {
    let resolveConvert: ((value: string) => void) | undefined;
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/photo.png"], category: "image" });
        case "hide_overlay":
          return Promise.resolve(undefined);
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        case "convert_file":
          return new Promise<string>((resolve) => {
            resolveConvert = resolve;
          });
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());

    // First wedge selection starts a conversion that stays in flight.
    await user.keyboard("{Enter}");
    expect(invokeMock.mock.calls.filter(([command]) => command === "convert_file")).toHaveLength(1);

    // A second selection while it runs must be ignored (the gate in
    // handleSelect): no second concurrent invoke racing the first toward
    // the same output path.
    await user.keyboard("{Enter}");
    expect(invokeMock.mock.calls.filter(([command]) => command === "convert_file")).toHaveLength(1);

    await act(async () => {
      resolveConvert?.("/home/user/photo.jpg");
    });
    await waitFor(() => expect(screen.queryByTestId("wedge-menu")).not.toBeInTheDocument());
  });

  it("does not let a stale conversion completion close a re-opened overlay", async () => {
    let resolveConvert: ((value: string) => void) | undefined;
    let emitLaunchRequest: ((event: { payload: unknown }) => void) | undefined;
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/photo.png"], category: "image" });
        case "hide_overlay":
          return Promise.resolve(undefined);
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        case "convert_file":
          return new Promise<string>((resolve) => {
            resolveConvert = resolve;
          });
        default:
          return Promise.resolve(undefined);
      }
    });
    listenMock.mockImplementation((_name: string, handler: (event: { payload: unknown }) => void) => {
      emitLaunchRequest = handler;
      return Promise.resolve(() => {});
    });

    render(<OverlayApp />);
    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());

    // Session 1: pick a wedge (conversion stays in flight), then dismiss
    // the overlay while it still runs.
    await user.keyboard("{Enter}");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByTestId("wedge-menu")).not.toBeInTheDocument());

    // A fresh trigger re-opens the overlay before the first conversion
    // completes — a brand-new session for a different file.
    act(() => {
      emitLaunchRequest?.({ payload: { mode: "formats", paths: ["/home/user/other.png"], category: "image" } });
    });
    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());

    const hideCallsBefore = invokeMock.mock.calls.filter(([command]) => command === "hide_overlay").length;

    // The stale first-session conversion completes now.
    await act(async () => {
      resolveConvert?.("/home/user/photo.jpg");
    });

    // Its completion must not close the re-opened overlay (the same
    // stale-async-continuation hide race as the Rust-side single-instance
    // fix), nor surface its result/error in the new session.
    expect(screen.getByTestId("wedge-menu")).toBeInTheDocument();
    expect(invokeMock.mock.calls.filter(([command]) => command === "hide_overlay").length).toBe(hideCallsBefore);
    expect(screen.queryByTestId("overlay-feedback")).not.toBeInTheDocument();
  });

  it("clears the converting gate on failure so the wedge is selectable again", async () => {
    let rejectConvert: ((error: Error) => void) | undefined;
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/photo.png"], category: "image" });
        case "hide_overlay":
          return Promise.resolve(undefined);
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        case "convert_file":
          return new Promise<string>((_resolve, reject) => {
            rejectConvert = reject;
          });
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());

    // Start a conversion, fail it.
    await user.keyboard("{Enter}");
    await act(async () => {
      rejectConvert?.(new Error("boom"));
    });
    await screen.findByTestId("overlay-feedback");
    expect(screen.getByTestId("wedge-menu")).toBeInTheDocument();

    // The gate must be cleared on failure: a fresh selection fires a new
    // conversion instead of being swallowed as "already converting".
    await user.keyboard("{Enter}");
    await waitFor(() => expect(invokeMock.mock.calls.filter(([command]) => command === "convert_file")).toHaveLength(2));
  });

  it("shows an in-flight converting indicator and clears it on completion", async () => {
    let resolveConvert: ((value: string) => void) | undefined;
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/photo.png"], category: "image" });
        case "hide_overlay":
          return Promise.resolve(undefined);
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        case "convert_file":
          return new Promise<string>((resolve) => {
            resolveConvert = resolve;
          });
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());

    // While the conversion is in flight, an informational banner names the
    // work being done — the menu must not look frozen during a long video
    // conversion.
    await user.keyboard("{Enter}");
    const indicator = await screen.findByTestId("converting-feedback");
    expect(indicator).toHaveTextContent("Converting to JPG…");
    expect(screen.getByTestId("wedge-menu")).toBeInTheDocument();

    // Completion clears the indicator and closes the overlay (unchanged).
    await act(async () => {
      resolveConvert?.("/home/user/photo.jpg");
    });
    await waitFor(() => expect(screen.queryByTestId("wedge-menu")).not.toBeInTheDocument());
    expect(screen.queryByTestId("converting-feedback")).not.toBeInTheDocument();
  });

  it("clears the converting indicator on failure, superseded by the error banner", async () => {
    let rejectConvert: ((error: Error) => void) | undefined;
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/photo.png"], category: "image" });
        case "hide_overlay":
          return Promise.resolve(undefined);
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        case "convert_file":
          return new Promise<string>((_resolve, reject) => {
            rejectConvert = reject;
          });
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());

    await user.keyboard("{Enter}");
    await screen.findByTestId("converting-feedback");

    await act(async () => {
      rejectConvert?.(new Error("boom"));
    });
    // The failure supersedes the in-flight indicator with the error banner.
    const error = await screen.findByTestId("overlay-feedback");
    expect(error).toHaveTextContent("Convert to JPG failed");
    expect(screen.queryByTestId("converting-feedback")).not.toBeInTheDocument();
  });

  it("does not write a file for an advanced-tool selection", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "tools", paths: ["/home/user/clip.mp4"], category: "video" });
        case "hide_overlay":
          return Promise.resolve(undefined);
        case "list_conversion_targets":
          return Promise.resolve([]);
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());

    await user.keyboard("{Enter}");

    await waitFor(() => expect(screen.queryByTestId("wedge-menu")).not.toBeInTheDocument());
    expect(invokeMock).not.toHaveBeenCalledWith("convert_file", expect.anything());
  });

  it("live-toggles to tools mode when Alt is pressed after the menu opens, and back on release", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/photo.png"], category: "image" });
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());
    expect(screen.getByRole("menu")).toHaveAccessibleName("Convert to format");

    await user.keyboard("{Alt>}");
    await waitFor(() => expect(screen.getByRole("menu")).toHaveAccessibleName("Advanced tools"));

    await user.keyboard("{/Alt}");
    await waitFor(() => expect(screen.getByRole("menu")).toHaveAccessibleName("Convert to format"));
  });

  it("live-toggles to tools mode via a configured extra key, not just Alt", async () => {
    const user = userEvent.setup();
    const settings: Settings = {
      formatModifiers: DEFAULT_FORMAT_MODIFIERS,
      toolsModifiers: { ctrl: false, alt: false, shift: true, meta: false, key: "KeyP" },
      activeThemeId: "citrus",
      colorOverrides: null,
      ffmpegPath: null,
      pdfiumPath: null,
    };
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/photo.png"], category: "image" });
        case "get_settings":
          return Promise.resolve(settings);
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());
    expect(screen.getByRole("menu")).toHaveAccessibleName("Convert to format");

    await user.keyboard("{p>}");
    await waitFor(() => expect(screen.getByRole("menu")).toHaveAccessibleName("Advanced tools"));

    await user.keyboard("{/p}");
    await waitFor(() => expect(screen.getByRole("menu")).toHaveAccessibleName("Convert to format"));
  });

  it("does not flip a dedicated tools-mode launch request back to formats just because Alt isn't held", async () => {
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "tools", paths: ["/home/user/clip.mp4"], category: "video" });
        case "list_conversion_targets":
          return Promise.resolve([]);
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());

    // A "Satsuma Tools" context-menu click sets tools mode explicitly, with
    // no modifier held at all — it must not immediately snap back to
    // formats mode just because Alt reads as not-held at that instant.
    expect(screen.getByRole("menu")).toHaveAccessibleName("Advanced tools");
  });

  it("hides the overlay window on Escape without acting", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/photo.png"], category: "image" });
        case "hide_overlay":
          return Promise.resolve(undefined);
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());

    await user.keyboard("{Escape}");

    await waitFor(() => expect(screen.queryByTestId("wedge-menu")).not.toBeInTheDocument());
    expect(invokeMock).toHaveBeenCalledWith("hide_overlay");
  });

  it("hides the overlay on Escape even when it opened with no renderable options", async () => {
    // Belt-and-suspenders: this shouldn't normally happen now that the
    // Rust side skips opening the overlay at all for an unsupported file
    // (category "unknown"), but if it ever does, the overlay must still be
    // dismissable rather than becoming a permanent, invisible, click-eating
    // window — see the OverlayApp-level Escape listener, independent of
    // WedgeMenu's own (which only attaches once there are options to show).
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/archive.zip"], category: "unknown" });
        case "hide_overlay":
          return Promise.resolve(undefined);
        case "list_conversion_targets":
          return Promise.resolve([]);
        default:
          return Promise.resolve(undefined);
      }
    });
    const user = userEvent.setup();

    render(<OverlayApp />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("take_launch_request"));
    expect(screen.queryByTestId("wedge-menu")).not.toBeInTheDocument();

    await user.keyboard("{Escape}");

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("hide_overlay"));
  });

  it("keeps the overlay open and shows an inline error banner when a conversion fails", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/photo.png"], category: "image" });
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        case "convert_file":
          return Promise.reject(new Error("FFmpeg sidecar not found"));
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());

    await user.keyboard("{Enter}");

    const feedback = await screen.findByTestId("overlay-feedback");
    expect(feedback).toHaveAttribute("role", "alert");
    expect(feedback).toHaveTextContent("Convert to JPG failed for photo.png: Error: FFmpeg sidecar not found");
    // A failed conversion must not silently close the glass: the menu
    // stays up so the user can retry.
    expect(screen.getByTestId("wedge-menu")).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("hide_overlay");
  });

  it("keeps the overlay open and shows an inline error banner when extraction fails", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "tools", paths: ["/home/user/bundle.zip"], category: "archive" });
        case "extract_archive":
          return Promise.reject(new Error("not a valid ZIP file"));
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());

    await user.keyboard("{Enter}");

    const feedback = await screen.findByTestId("overlay-feedback");
    expect(feedback).toHaveTextContent("Extract failed for bundle.zip: Error: not a valid ZIP file");
    expect(screen.getByTestId("wedge-menu")).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalledWith("hide_overlay");
  });

  it("closes the overlay when extraction succeeds", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "tools", paths: ["/home/user/bundle.zip"], category: "archive" });
        case "extract_archive":
          return Promise.resolve("C:\\home\\user\\bundle Extracted");
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());

    await user.keyboard("{Enter}");

    await waitFor(() => expect(screen.queryByTestId("wedge-menu")).not.toBeInTheDocument());
    expect(invokeMock).toHaveBeenCalledWith("extract_archive", { path: "/home/user/bundle.zip" });
    expect(invokeMock).toHaveBeenCalledWith("hide_overlay");
  });

  it("clears a stale error banner on the next launch request", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/photo.png"], category: "image" });
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        case "convert_file":
          return Promise.reject(new Error("boom"));
        default:
          return Promise.resolve(undefined);
      }
    });
    let emitLaunchRequest: ((event: { payload: unknown }) => void) | undefined;
    listenMock.mockImplementation((_name: string, handler: (event: { payload: unknown }) => void) => {
      emitLaunchRequest = handler;
      return Promise.resolve(() => {});
    });

    render(<OverlayApp />);
    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());

    await user.keyboard("{Enter}");
    await screen.findByTestId("overlay-feedback");

    act(() => {
      emitLaunchRequest?.({ payload: { mode: "formats", paths: ["/home/user/other.png"], category: "image" } });
    });

    await waitFor(() => expect(screen.queryByTestId("overlay-feedback")).not.toBeInTheDocument());
    expect(screen.getByTestId("wedge-menu")).toBeInTheDocument();
  });

  it("requests OS/window focus once the overlay is open, so keys reach the page without a click", async () => {
    const focusSpy = vi.spyOn(window, "focus");
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/photo.png"], category: "image" });
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    expect(focusSpy).not.toHaveBeenCalled();

    await waitFor(() => expect(screen.getByTestId("wedge-menu")).toBeInTheDocument());
    expect(focusSpy).toHaveBeenCalledTimes(1);
  });

  it("stays open and Escape-dismissable when the conversion-target list fails to load", async () => {
    const user = userEvent.setup();
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "take_launch_request":
          return Promise.resolve({ mode: "formats", paths: ["/home/user/photo.png"], category: "image" });
        case "list_conversion_targets":
          return Promise.reject(new Error("backend error"));
        default:
          return Promise.resolve(undefined);
      }
    });

    render(<OverlayApp />);
    // The launch still opens the overlay (the backend target fetch failed,
    // so no wedge options render), and it must still be dismissable —
    // never a stuck, invisible, click-catching window.
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("list_conversion_targets", expect.anything()));
    expect(screen.queryByTestId("wedge-menu")).not.toBeInTheDocument();

    await user.keyboard("{Escape}");

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("hide_overlay"));
  });
});
