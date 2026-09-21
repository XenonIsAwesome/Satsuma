import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { LinuxIntegrationPanel } from "./LinuxIntegrationPanel";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

function mockInvoke(overrides: Record<string, unknown> = {}) {
  invokeMock.mockImplementation((command: string) => {
    if (command in overrides) return Promise.resolve(overrides[command]);
    switch (command) {
      case "current_platform":
        return Promise.resolve("linux");
      case "is_linux_file_manager_integration_installed":
        return Promise.resolve(false);
      case "install_linux_file_manager_integration":
      case "uninstall_linux_file_manager_integration":
        return Promise.resolve(undefined);
      default:
        return Promise.resolve(undefined);
    }
  });
}

describe("LinuxIntegrationPanel", () => {
  beforeEach(() => {
    mockInvoke();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders nothing on non-Linux platforms", async () => {
    mockInvoke({ current_platform: "windows" });
    render(<LinuxIntegrationPanel />);

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("current_platform"));
    expect(screen.queryByTestId("linux-integration-panel")).not.toBeInTheDocument();
  });

  it("shows disabled status when not installed", async () => {
    render(<LinuxIntegrationPanel />);
    expect(await screen.findByTestId("linux-integration-panel")).toBeInTheDocument();
    expect(screen.getByText("disabled")).toBeInTheDocument();
    expect(screen.getByTestId("linux-integration-toggle")).toHaveTextContent("Add context menu entries");
  });

  it("shows enabled status when already installed", async () => {
    mockInvoke({ is_linux_file_manager_integration_installed: true });
    render(<LinuxIntegrationPanel />);
    expect(await screen.findByText("enabled")).toBeInTheDocument();
    expect(screen.getByTestId("linux-integration-toggle")).toHaveTextContent("Remove context menu entries");
  });

  it("installs the integration when toggled on", async () => {
    const user = userEvent.setup();
    render(<LinuxIntegrationPanel />);
    await screen.findByTestId("linux-integration-panel");

    await user.click(screen.getByTestId("linux-integration-toggle"));

    await waitFor(() => expect(screen.getByText("enabled")).toBeInTheDocument());
    expect(invokeMock).toHaveBeenCalledWith("install_linux_file_manager_integration");
  });

  it("uninstalls the integration when toggled off", async () => {
    mockInvoke({ is_linux_file_manager_integration_installed: true });
    const user = userEvent.setup();
    render(<LinuxIntegrationPanel />);
    await screen.findByText("enabled");

    await user.click(screen.getByTestId("linux-integration-toggle"));

    await waitFor(() => expect(screen.getByText("disabled")).toBeInTheDocument());
    expect(invokeMock).toHaveBeenCalledWith("uninstall_linux_file_manager_integration");
  });

  it("shows an error message when the install command fails", async () => {
    invokeMock.mockImplementation((command: string) => {
      switch (command) {
        case "current_platform":
          return Promise.resolve("linux");
        case "is_linux_file_manager_integration_installed":
          return Promise.resolve(false);
        case "install_linux_file_manager_integration":
          return Promise.reject(new Error("permission denied"));
        default:
          return Promise.resolve(undefined);
      }
    });
    const user = userEvent.setup();
    render(<LinuxIntegrationPanel />);
    await screen.findByTestId("linux-integration-panel");

    await user.click(screen.getByTestId("linux-integration-toggle"));

    expect(await screen.findByTestId("linux-integration-error")).toHaveTextContent("permission denied");
    expect(screen.getByText("disabled")).toBeInTheDocument();
  });
});
