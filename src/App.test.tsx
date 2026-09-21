import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { DEFAULT_SETTINGS } from "./hooks/useSettings";

const { onDragDropEventMock, invokeMock, listenMock } = vi.hoisted(() => ({
  onDragDropEventMock: vi.fn(),
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({ onDragDropEvent: onDragDropEventMock }),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

describe("App", () => {
  let dragDropHandler: (event: { payload: Record<string, unknown> }) => void;

  beforeEach(() => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    listenMock.mockResolvedValue(() => {});
    onDragDropEventMock.mockImplementation((handler: typeof dragDropHandler) => {
      dragDropHandler = handler;
      return Promise.resolve(() => {});
    });
    invokeMock.mockImplementation((command: string, args?: Record<string, unknown>) => {
      switch (command) {
        case "detect_file_category":
          return Promise.resolve("image");
        case "current_platform":
          // Not "linux": keeps the LinuxIntegrationPanel out of these tests.
          return Promise.resolve("test");
        case "is_linux_file_manager_integration_installed":
          return Promise.resolve(false);
        case "list_conversion_targets":
          // Matches the real backend's shape for a .png source: every
          // raster target plus pdf/docx, excluding png itself.
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        case "convert_file":
          return Promise.resolve(`/home/user/photo.${String(args?.targetExtension)}`);
        case "get_settings":
          return Promise.resolve(DEFAULT_SETTINGS);
        default:
          return Promise.resolve(undefined);
      }
    });
  });

  afterEach(() => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    vi.clearAllMocks();
  });

  async function dropFile(path = "/home/user/photo.png") {
    act(() => {
      dragDropHandler({ payload: { type: "drop", paths: [path], position: { x: 0, y: 0 } } });
    });
    await waitFor(() => expect(screen.getByTestId("dropped-file-name")).toBeInTheDocument());
  }

  it("does not show the wedge menu before a file is dropped", () => {
    render(<App />);
    expect(screen.queryByTestId("wedge-menu")).not.toBeInTheDocument();
  });

  it("opens the format menu when Shift is held after a file is dropped", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dropFile();

    await user.keyboard("{Shift>}");
    expect(screen.getByTestId("wedge-menu")).toBeInTheDocument();
    expect(screen.getByRole("menu")).toHaveAccessibleName("Convert to format");
    await waitFor(() => expect(screen.getByTestId("wedge-jpg")).toBeInTheDocument());
  });

  it("switches to advanced-tools mode when Alt is also held", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dropFile();

    await user.keyboard("{Shift>}{Alt>}");
    expect(screen.getByRole("menu")).toHaveAccessibleName("Advanced tools");
    expect(screen.getByTestId("wedge-compress")).toBeInTheDocument();
  });

  it("opens the menu via Enter for keyboard-only use, and applies a selection", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dropFile();

    await user.keyboard("{Enter}");
    expect(screen.getByTestId("wedge-menu")).toBeInTheDocument();
    await waitFor(() => expect(screen.getByTestId("wedge-jpg")).toBeInTheDocument());

    await user.keyboard("{ArrowRight}{Enter}");
    expect(screen.queryByTestId("wedge-menu")).not.toBeInTheDocument();

    // Selecting a format wedge triggers a real conversion — the result
    // only appears once that async command resolves.
    await waitFor(() => expect(screen.getByTestId("last-action")).toBeInTheDocument());
    expect(invokeMock).toHaveBeenCalledWith("convert_file", {
      path: "/home/user/photo.png",
      targetExtension: "webp",
    });
    expect(screen.getByTestId("last-action")).toHaveTextContent("Convert to WebP");
    expect(screen.getByTestId("last-action")).toHaveTextContent("photo.webp");
    expect(screen.getByTestId("last-action")).toHaveClass("toast--success");
  });

  it("shows a persistent failure toast when the stub conversion fails", async () => {
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "detect_file_category":
          return Promise.resolve("image");
        case "current_platform":
          return Promise.resolve("test");
        case "is_linux_file_manager_integration_installed":
          return Promise.resolve(false);
        case "list_conversion_targets":
          return Promise.resolve(["jpg", "webp", "tiff", "avif", "bmp", "pdf", "docx"]);
        case "convert_file":
          return Promise.reject(new Error("disk full"));
        case "get_settings":
          return Promise.resolve(DEFAULT_SETTINGS);
        default:
          return Promise.resolve(undefined);
      }
    });
    const user = userEvent.setup();
    render(<App />);
    await dropFile();

    await user.keyboard("{Enter}");
    await waitFor(() => expect(screen.getByTestId("wedge-jpg")).toBeInTheDocument());
    await user.keyboard("{ArrowRight}{Enter}");

    await waitFor(() => expect(screen.getByTestId("last-action")).toBeInTheDocument());
    expect(screen.getByTestId("last-action")).toHaveTextContent("photo.png");
    expect(screen.getByTestId("last-action")).toHaveTextContent("disk full");
    expect(screen.getByTestId("last-action")).toHaveClass("toast--danger");
  });

  it("does not write a file for an advanced-tool selection", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dropFile();

    await user.keyboard("{Shift>}{Alt>}");
    expect(screen.getByRole("menu")).toHaveAccessibleName("Advanced tools");

    await user.keyboard("{Enter}");

    await waitFor(() => expect(screen.getByTestId("last-action")).toBeInTheDocument());
    expect(invokeMock).not.toHaveBeenCalledWith("convert_file", expect.anything());
    expect(screen.getByTestId("last-action")).toHaveClass("toast--info");
  });

  it("closes the menu without acting on Escape", async () => {
    const user = userEvent.setup();
    render(<App />);
    await dropFile();

    await user.keyboard("{Enter}");
    expect(screen.getByTestId("wedge-menu")).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByTestId("wedge-menu")).not.toBeInTheDocument();
    expect(screen.queryByTestId("last-action")).not.toBeInTheDocument();
  });
});
