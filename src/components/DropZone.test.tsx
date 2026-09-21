import { act, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DropZone } from "./DropZone";

const { onDragDropEventMock, invokeMock } = vi.hoisted(() => ({
  onDragDropEventMock: vi.fn(),
  invokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({ onDragDropEvent: onDragDropEventMock }),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

describe("DropZone", () => {
  let dragDropHandler: (event: { payload: Record<string, unknown> }) => void;

  beforeEach(() => {
    (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
    onDragDropEventMock.mockImplementation((handler: typeof dragDropHandler) => {
      dragDropHandler = handler;
      return Promise.resolve(() => {});
    });
    invokeMock.mockResolvedValue("image");
  });

  afterEach(() => {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    vi.clearAllMocks();
  });

  it("shows the idle hint when no file has been dropped", () => {
    render(<DropZone droppedFile={null} onFileDropped={vi.fn()} />);
    expect(screen.getByText(/drop a file here/i)).toBeInTheDocument();
  });

  it("adds the active class while a file is dragged over", () => {
    render(<DropZone droppedFile={null} onFileDropped={vi.fn()} />);
    act(() => {
      dragDropHandler({ payload: { type: "over", position: { x: 0, y: 0 } } });
    });
    expect(screen.getByTestId("drop-zone")).toHaveClass("drop-zone--active");
  });

  it("clears the active class on leave", () => {
    render(<DropZone droppedFile={null} onFileDropped={vi.fn()} />);
    act(() => {
      dragDropHandler({ payload: { type: "over", position: { x: 0, y: 0 } } });
    });
    act(() => {
      dragDropHandler({ payload: { type: "leave" } });
    });
    expect(screen.getByTestId("drop-zone")).not.toHaveClass("drop-zone--active");
  });

  it("detects the category via the backend and reports the dropped file", async () => {
    const onFileDropped = vi.fn();
    render(<DropZone droppedFile={null} onFileDropped={onFileDropped} />);

    act(() => {
      dragDropHandler({
        payload: { type: "drop", paths: ["/home/user/photo.png"], position: { x: 0, y: 0 } },
      });
    });

    await waitFor(() =>
      expect(onFileDropped).toHaveBeenCalledWith({
        path: "/home/user/photo.png",
        name: "photo.png",
        category: "image",
      }),
    );
    expect(invokeMock).toHaveBeenCalledWith("detect_file_category", { path: "/home/user/photo.png" });
  });

  it("shows the dropped file name and category once provided", () => {
    render(
      <DropZone
        droppedFile={{ path: "/a/b/clip.mp4", name: "clip.mp4", category: "video" }}
        onFileDropped={vi.fn()}
      />,
    );
    expect(screen.getByTestId("dropped-file-name")).toHaveTextContent("clip.mp4");
    expect(screen.getByTestId("dropped-file-category")).toHaveTextContent("video");
  });
});
