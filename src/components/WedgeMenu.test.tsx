import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { WedgeMenu } from "./WedgeMenu";
import type { WedgeOption } from "../types";

const OPTIONS: WedgeOption[] = [
  { id: "jpg", label: "JPG", icon: "🖼️" },
  { id: "png", label: "PNG", icon: "🖼️" },
  { id: "webp", label: "WebP", icon: "🖼️" },
];

describe("WedgeMenu", () => {
  beforeEach(() => {
    // The modifier-key hint badge is only shown the first few times per
    // install (tracked in localStorage) — reset it so each test starts
    // from a deterministic "never shown" state.
    window.localStorage.clear();
  });

  it("renders nothing when closed", () => {
    render(
      <WedgeMenu open={false} mode="formats" options={OPTIONS} onSelect={vi.fn()} onCancel={vi.fn()} />,
    );
    expect(screen.queryByTestId("wedge-menu")).not.toBeInTheDocument();
  });

  it("renders nothing when there are no options", () => {
    render(<WedgeMenu open={true} mode="formats" options={[]} onSelect={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.queryByTestId("wedge-menu")).not.toBeInTheDocument();
  });

  it("renders a wedge per option when open", () => {
    render(
      <WedgeMenu open={true} mode="formats" options={OPTIONS} onSelect={vi.fn()} onCancel={vi.fn()} />,
    );
    expect(screen.getByTestId("wedge-jpg")).toBeInTheDocument();
    expect(screen.getByTestId("wedge-png")).toBeInTheDocument();
    expect(screen.getByTestId("wedge-webp")).toBeInTheDocument();
  });

  it("labels the menu according to mode", () => {
    const { rerender } = render(
      <WedgeMenu open={true} mode="formats" options={OPTIONS} onSelect={vi.fn()} onCancel={vi.fn()} />,
    );
    expect(screen.getByRole("menu")).toHaveAccessibleName("Convert to format");

    rerender(
      <WedgeMenu open={true} mode="tools" options={OPTIONS} onSelect={vi.fn()} onCancel={vi.fn()} />,
    );
    expect(screen.getByRole("menu")).toHaveAccessibleName("Advanced tools");
  });

  it("starts with the first wedge selected", () => {
    render(
      <WedgeMenu open={true} mode="formats" options={OPTIONS} onSelect={vi.fn()} onCancel={vi.fn()} />,
    );
    expect(screen.getByTestId("wedge-jpg")).toHaveAttribute("aria-checked", "true");
    expect(screen.getByTestId("wedge-png")).toHaveAttribute("aria-checked", "false");
  });

  it("moves selection forward with ArrowRight and wraps around", async () => {
    const user = userEvent.setup();
    render(
      <WedgeMenu open={true} mode="formats" options={OPTIONS} onSelect={vi.fn()} onCancel={vi.fn()} />,
    );

    await user.keyboard("{ArrowRight}");
    expect(screen.getByTestId("wedge-png")).toHaveAttribute("aria-checked", "true");

    await user.keyboard("{ArrowRight}{ArrowRight}");
    expect(screen.getByTestId("wedge-jpg")).toHaveAttribute("aria-checked", "true");
  });

  it("moves selection backward with ArrowLeft and wraps around", async () => {
    const user = userEvent.setup();
    render(
      <WedgeMenu open={true} mode="formats" options={OPTIONS} onSelect={vi.fn()} onCancel={vi.fn()} />,
    );

    await user.keyboard("{ArrowLeft}");
    expect(screen.getByTestId("wedge-webp")).toHaveAttribute("aria-checked", "true");
  });

  it("applies the selected option on Enter", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(
      <WedgeMenu open={true} mode="formats" options={OPTIONS} onSelect={onSelect} onCancel={vi.fn()} />,
    );

    await user.keyboard("{ArrowRight}{Enter}");
    expect(onSelect).toHaveBeenCalledWith(OPTIONS[1]);
  });

  it("cancels on Escape", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    render(
      <WedgeMenu open={true} mode="formats" options={OPTIONS} onSelect={vi.fn()} onCancel={onCancel} />,
    );

    await user.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("selects an option on click", async () => {
    const user = userEvent.setup();
    const onSelect = vi.fn();
    render(
      <WedgeMenu open={true} mode="formats" options={OPTIONS} onSelect={onSelect} onCancel={vi.fn()} />,
    );

    await user.click(screen.getByTestId("wedge-webp"));
    expect(onSelect).toHaveBeenCalledWith(OPTIONS[2]);
  });

  it("cancels when clicking the overlay background", async () => {
    const user = userEvent.setup();
    const onCancel = vi.fn();
    render(
      <WedgeMenu open={true} mode="formats" options={OPTIONS} onSelect={vi.fn()} onCancel={onCancel} />,
    );

    await user.click(screen.getByTestId("wedge-menu"));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("shows the source file's format in the hub, never as a selectable target", () => {
    render(
      <WedgeMenu
        open={true}
        mode="formats"
        options={OPTIONS}
        onSelect={vi.fn()}
        onCancel={vi.fn()}
        hubLabel="HEIC"
      />,
    );
    expect(screen.getByTestId("wedge-hub-label")).toHaveTextContent("HEIC");
  });

  it("renders no hub label when none is given", () => {
    render(<WedgeMenu open={true} mode="formats" options={OPTIONS} onSelect={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.queryByTestId("wedge-hub-label")).not.toBeInTheDocument();
  });

  it("echoes the hovered option in the live label, updating on selection change", async () => {
    const user = userEvent.setup();
    render(<WedgeMenu open={true} mode="formats" options={OPTIONS} onSelect={vi.fn()} onCancel={vi.fn()} />);

    expect(screen.getByTestId("wedge-live-label")).toHaveTextContent("Convert to JPG");

    await user.keyboard("{ArrowRight}");
    expect(screen.getByTestId("wedge-live-label")).toHaveTextContent("Convert to PNG");
  });

  it("shows the plain option label (no 'Convert to') in the live label for tools mode", () => {
    render(<WedgeMenu open={true} mode="tools" options={OPTIONS} onSelect={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByTestId("wedge-live-label")).toHaveTextContent("JPG");
    expect(screen.getByTestId("wedge-live-label")).not.toHaveTextContent("Convert to");
  });

  it("shows the Alt hint badge in formats mode the first few times, then stops", () => {
    const { unmount } = render(
      <WedgeMenu open={true} mode="formats" options={OPTIONS} onSelect={vi.fn()} onCancel={vi.fn()} />,
    );
    expect(screen.getByTestId("modifier-hint-badge")).toBeInTheDocument();
    unmount();

    for (let i = 0; i < 5; i += 1) {
      const rendered = render(
        <WedgeMenu open={true} mode="formats" options={OPTIONS} onSelect={vi.fn()} onCancel={vi.fn()} />,
      );
      rendered.unmount();
    }

    render(<WedgeMenu open={true} mode="formats" options={OPTIONS} onSelect={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.queryByTestId("modifier-hint-badge")).not.toBeInTheDocument();
  });

  it("never shows the Alt hint badge in tools mode", () => {
    render(<WedgeMenu open={true} mode="tools" options={OPTIONS} onSelect={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.queryByTestId("modifier-hint-badge")).not.toBeInTheDocument();
  });

  it("shows the 'Not implemented yet' note in tools mode when unimplementedTools is provided", () => {
    render(
      <WedgeMenu
        open={true}
        mode="tools"
        options={OPTIONS}
        onSelect={vi.fn()}
        onCancel={vi.fn()}
        unimplementedTools="compress, crop"
      />,
    );
    expect(screen.getByText("Not implemented yet:")).toBeInTheDocument();
    expect(screen.getByText("compress, crop")).toBeInTheDocument();
  });

  it("does not show the 'Not implemented yet' note in formats mode even if unimplementedTools is provided", () => {
    render(
      <WedgeMenu
        open={true}
        mode="formats"
        options={OPTIONS}
        onSelect={vi.fn()}
        onCancel={vi.fn()}
        unimplementedTools="compress, crop"
      />,
    );
    expect(screen.queryByText("Not implemented yet:")).not.toBeInTheDocument();
  });

  it("does not show the 'Not implemented yet' note when unimplementedTools is undefined", () => {
    render(
      <WedgeMenu
        open={true}
        mode="tools"
        options={OPTIONS}
        onSelect={vi.fn()}
        onCancel={vi.fn()}
        unimplementedTools={undefined}
      />,
    );
    expect(screen.queryByText("Not implemented yet:")).not.toBeInTheDocument();
  });
});
