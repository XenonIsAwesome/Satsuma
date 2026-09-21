import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";
import { SettingsScreen } from "./SettingsScreen";
import { DEFAULT_SETTINGS } from "../hooks/useSettings";
import { createModifierKeyDispatcher } from "../test/modifierEvents";
import type { Settings, ThemeFile } from "../types";

const { invokeMock, openFileDialogMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  openFileDialogMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: openFileDialogMock,
}));

const CITRUS: ThemeFile = {
  id: "citrus",
  name: "Citrus",
  colors: { primary: "#ff7a29", secondary: "#2b2620", neutral: "#f6f3ee", surface: "#ffffff", outline: "#eae4d9" },
};
const YUZU: ThemeFile = {
  id: "yuzu",
  name: "Yuzu",
  colors: { primary: "#d4a017", secondary: "#26241c", neutral: "#f7f5ec", surface: "#ffffff", outline: "#e8e2d0" },
};
const THEMES = [CITRUS, YUZU];

function mockInvoke(overrides: Record<string, unknown> = {}) {
  invokeMock.mockImplementation((command: string) => {
    if (command in overrides) return Promise.resolve(overrides[command]);
    switch (command) {
      case "current_platform":
        return Promise.resolve("windows");
      case "is_autostart_enabled":
        return Promise.resolve(false);
      case "set_autostart_enabled":
        return Promise.resolve(undefined);
      case "save_theme":
        return Promise.resolve({ id: "new-theme", name: "New Theme", colors: CITRUS.colors });
      case "themes_dir_display":
        return Promise.resolve("C:\\Users\\alice\\AppData\\Roaming\\satsuma\\themes");
      case "is_linux_file_manager_integration_installed":
        return Promise.resolve(false);
      default:
        return Promise.resolve(undefined);
    }
  });
}

describe("SettingsScreen", () => {
  let onUpdate: Mock<(next: Settings) => Promise<void>>;
  let onThemesChange: Mock<() => Promise<ThemeFile[]>>;
  let onClose: Mock<() => void>;

  beforeEach(() => {
    mockInvoke();
    onUpdate = vi.fn<(next: Settings) => Promise<void>>().mockResolvedValue(undefined);
    onThemesChange = vi.fn<() => Promise<ThemeFile[]>>().mockResolvedValue(THEMES);
    onClose = vi.fn<() => void>();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  function renderScreen(settings: Settings = DEFAULT_SETTINGS, themes: ThemeFile[] = THEMES) {
    return render(
      <SettingsScreen
        settings={settings}
        themes={themes}
        onUpdate={onUpdate}
        onThemesChange={onThemesChange}
        onClose={onClose}
      />,
    );
  }

  it("renders the current hotkey bindings", () => {
    renderScreen();
    expect(screen.getByTestId("format-hotkey-button")).toHaveTextContent("Shift");
    expect(screen.getByTestId("tools-hotkey-button")).toHaveTextContent("Shift+Alt");
  });

  it("calls onClose when Back is clicked", async () => {
    const user = userEvent.setup();
    renderScreen();
    await user.click(screen.getByTestId("settings-close"));
    expect(onClose).toHaveBeenCalled();
  });

  it("applies a valid recorded hotkey immediately", async () => {
    const user = userEvent.setup();
    renderScreen();

    await user.click(screen.getByTestId("format-hotkey-button"));
    const dispatcher = createModifierKeyDispatcher();
    dispatcher.down("Control");
    dispatcher.up("Control");

    await waitFor(() =>
      expect(onUpdate).toHaveBeenCalledWith({
        ...DEFAULT_SETTINGS,
        formatModifiers: { ctrl: true, alt: false, shift: false, meta: false, key: null },
      }),
    );
  });

  it("rejects making the two bindings identical without saving", async () => {
    const user = userEvent.setup();
    renderScreen();

    // Tools starts as Shift+Alt; recording plain Shift for it makes it the
    // same as the format binding.
    await user.click(screen.getByTestId("tools-hotkey-button"));
    const dispatcher = createModifierKeyDispatcher();
    dispatcher.down("Shift");
    dispatcher.up("Shift");

    expect(await screen.findByTestId("hotkey-error")).toHaveTextContent(/identical/);
    expect(onUpdate).not.toHaveBeenCalled();
  });

  it("warns about a reserved bare-Win combination without blocking it", async () => {
    const user = userEvent.setup();
    renderScreen();

    await user.click(screen.getByTestId("format-hotkey-button"));
    const dispatcher = createModifierKeyDispatcher();
    dispatcher.down("Meta");
    dispatcher.up("Meta");

    expect(await screen.findByTestId("hotkey-warning-format")).toHaveTextContent(/Win/);
    expect(onUpdate).toHaveBeenCalledWith({
      ...DEFAULT_SETTINGS,
      formatModifiers: { ctrl: false, alt: false, shift: false, meta: true, key: null },
    });
  });

  it("resets hotkeys to their defaults without touching the theme", async () => {
    const user = userEvent.setup();
    const customized: Settings = {
      formatModifiers: { ctrl: true, alt: false, shift: false, meta: false, key: null },
      toolsModifiers: { ctrl: true, alt: true, shift: false, meta: false, key: null },
      activeThemeId: "yuzu",
      colorOverrides: { ...CITRUS.colors, primary: "#123456" },
      ffmpegPath: null,
      pdfiumPath: null,
    };
    renderScreen(customized);
    expect(screen.getByTestId("format-hotkey-button")).toHaveTextContent("Ctrl");

    await user.click(screen.getByTestId("reset-hotkeys"));

    expect(onUpdate).toHaveBeenCalledWith({
      ...customized,
      formatModifiers: { ctrl: false, alt: false, shift: true, meta: false, key: null },
      toolsModifiers: { ctrl: false, alt: true, shift: true, meta: false, key: null },
    });
    expect(screen.getByTestId("format-hotkey-button")).toHaveTextContent("Shift");
    expect(screen.getByTestId("tools-hotkey-button")).toHaveTextContent("Shift+Alt");
  });

  it("clears a hotkey validation error on reset", async () => {
    const user = userEvent.setup();
    renderScreen();

    await user.click(screen.getByTestId("tools-hotkey-button"));
    const dispatcher = createModifierKeyDispatcher();
    dispatcher.down("Shift");
    dispatcher.up("Shift");
    expect(await screen.findByTestId("hotkey-error")).toBeInTheDocument();

    await user.click(screen.getByTestId("reset-hotkeys"));

    expect(screen.queryByTestId("hotkey-error")).not.toBeInTheDocument();
  });

  it("lists themes in the dropdown and selects one, clearing overrides", async () => {
    const user = userEvent.setup();
    const withOverride: Settings = {
      ...DEFAULT_SETTINGS,
      activeThemeId: "citrus",
      colorOverrides: { ...CITRUS.colors, primary: "#000000" },
    };
    renderScreen(withOverride);

    expect(screen.getByRole("option", { name: "Citrus" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Yuzu" })).toBeInTheDocument();

    await user.selectOptions(screen.getByTestId("theme-select"), "yuzu");

    expect(onUpdate).toHaveBeenCalledWith({ ...withOverride, activeThemeId: "yuzu", colorOverrides: null });
  });

  it("hides the color pickers and save-as row when a saved theme is selected", () => {
    renderScreen({ ...DEFAULT_SETTINGS, activeThemeId: "yuzu" });
    expect(screen.queryByTestId("theme-colors")).not.toBeInTheDocument();
    expect(screen.queryByTestId("new-theme-name")).not.toBeInTheDocument();
    expect(screen.getByTestId("theme-select")).toHaveValue("yuzu");
  });

  it("reveals the color pickers, seeded from the active theme, when Custom is picked", async () => {
    const user = userEvent.setup();
    renderScreen({ ...DEFAULT_SETTINGS, activeThemeId: "yuzu" });

    await user.selectOptions(screen.getByTestId("theme-select"), "custom");

    expect(onUpdate).toHaveBeenCalledWith({
      ...DEFAULT_SETTINGS,
      activeThemeId: "yuzu",
      colorOverrides: YUZU.colors,
    });
  });

  it("shows override colors and keeps Custom selected once colorOverrides is set", () => {
    renderScreen({
      ...DEFAULT_SETTINGS,
      activeThemeId: "citrus",
      colorOverrides: { ...CITRUS.colors, primary: "#123456" },
    });
    expect(screen.getByTestId("theme-select")).toHaveValue("custom");
    expect(screen.getByTestId("theme-color-primary")).toHaveValue("#123456");
    expect(screen.getByTestId("new-theme-name")).toBeInTheDocument();
  });

  it("applies a color tweak as a live override without changing the active theme id", async () => {
    renderScreen({
      ...DEFAULT_SETTINGS,
      activeThemeId: "citrus",
      colorOverrides: CITRUS.colors,
    });

    fireEvent.change(screen.getByTestId("theme-color-primary"), { target: { value: "#123456" } });

    await waitFor(() =>
      expect(onUpdate).toHaveBeenCalledWith({
        ...DEFAULT_SETTINGS,
        activeThemeId: "citrus",
        colorOverrides: { ...CITRUS.colors, primary: "#123456" },
      }),
    );
  });

  it("shows the real, OS-resolved themes directory path in the hint text", async () => {
    renderScreen();
    expect(await screen.findByText(/AppData\\Roaming\\satsuma\\themes/)).toBeInTheDocument();
  });

  it("opens the themes directory when the button is clicked", async () => {
    const user = userEvent.setup();
    renderScreen();

    await user.click(screen.getByTestId("open-themes-dir"));

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("open_themes_dir"));
    expect(screen.queryByTestId("open-themes-dir-error")).not.toBeInTheDocument();
  });

  it("shows an error if opening the themes directory fails", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "open_themes_dir") return Promise.reject(new Error("no file manager found"));
      if (command === "current_platform") return Promise.resolve("windows");
      if (command === "is_autostart_enabled") return Promise.resolve(false);
      return Promise.resolve(undefined);
    });
    const user = userEvent.setup();
    renderScreen();

    await user.click(screen.getByTestId("open-themes-dir"));

    expect(await screen.findByTestId("open-themes-dir-error")).toHaveTextContent("no file manager found");
  });

  it("saves the current colors as a new theme and switches to it", async () => {
    const user = userEvent.setup();
    const withOverride: Settings = {
      ...DEFAULT_SETTINGS,
      activeThemeId: "citrus",
      colorOverrides: { ...CITRUS.colors, primary: "#123456" },
    };
    renderScreen(withOverride);

    await user.type(screen.getByTestId("new-theme-name"), "My Theme");
    await user.click(screen.getByTestId("save-theme"));

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith("save_theme", {
        name: "My Theme",
        colors: { ...CITRUS.colors, primary: "#123456" },
      }),
    );
    expect(onThemesChange).toHaveBeenCalled();
    expect(onUpdate).toHaveBeenCalledWith({ ...withOverride, activeThemeId: "new-theme", colorOverrides: null });
  });

  it("shows an error if saving the theme fails", async () => {
    invokeMock.mockImplementation((command: string) => {
      if (command === "save_theme") return Promise.reject(new Error("disk full"));
      if (command === "current_platform") return Promise.resolve("windows");
      if (command === "is_autostart_enabled") return Promise.resolve(false);
      return Promise.resolve(undefined);
    });
    const user = userEvent.setup();
    renderScreen({ ...DEFAULT_SETTINGS, colorOverrides: CITRUS.colors });

    await user.type(screen.getByTestId("new-theme-name"), "Broken");
    await user.click(screen.getByTestId("save-theme"));

    expect(await screen.findByTestId("save-theme-error")).toHaveTextContent("disk full");
  });

  it("loads and toggles Start at Login", async () => {
    mockInvoke({ is_autostart_enabled: true });
    const user = userEvent.setup();
    renderScreen();

    const toggle = await screen.findByTestId("autostart-toggle");
    await waitFor(() => expect(toggle).toBeChecked());

    await user.click(toggle);

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("set_autostart_enabled", { enabled: false }));
  });

  it("renders the Linux integration panel on Linux", async () => {
    mockInvoke({ current_platform: "linux" });
    renderScreen();
    expect(await screen.findByTestId("linux-integration-panel")).toBeInTheDocument();
  });

  describe("external tools", () => {
    it("shows a not-found status by default (nothing bundled/vendored in a test double)", async () => {
      mockInvoke({ ffmpeg_status: null, pdfium_status: null });
      renderScreen();

      expect(await screen.findByTestId("ffmpeg-status")).toHaveTextContent("Not found");
      expect(await screen.findByTestId("pdfium-status")).toHaveTextContent("Not found");
    });

    it("shows the resolved path when the backend reports one", async () => {
      mockInvoke({ ffmpeg_status: "/usr/bin/ffmpeg", pdfium_status: "/opt/pdfium/libpdfium.so" });
      renderScreen();

      expect(await screen.findByTestId("ffmpeg-status")).toHaveTextContent("/usr/bin/ffmpeg");
      expect(await screen.findByTestId("pdfium-status")).toHaveTextContent("/opt/pdfium/libpdfium.so");
    });

    it("commits a typed FFmpeg path on blur", async () => {
      const user = userEvent.setup();
      renderScreen();

      const input = screen.getByTestId("ffmpeg-path-input");
      await user.type(input, "/opt/ffmpeg/bin/ffmpeg");
      await user.tab();

      expect(onUpdate).toHaveBeenCalledWith({ ...DEFAULT_SETTINGS, ffmpegPath: "/opt/ffmpeg/bin/ffmpeg" });
    });

    it("treats a blank FFmpeg path as clearing the override", async () => {
      const user = userEvent.setup();
      renderScreen({ ...DEFAULT_SETTINGS, ffmpegPath: "/opt/ffmpeg/bin/ffmpeg" });

      const input = screen.getByTestId("ffmpeg-path-input");
      await user.clear(input);
      await user.tab();

      expect(onUpdate).toHaveBeenCalledWith({ ...DEFAULT_SETTINGS, ffmpegPath: null });
    });

    it("sets the FFmpeg path from the native file picker", async () => {
      openFileDialogMock.mockResolvedValue("/home/user/ffmpeg");
      const user = userEvent.setup();
      renderScreen();

      await user.click(screen.getByTestId("ffmpeg-browse"));

      await waitFor(() =>
        expect(onUpdate).toHaveBeenCalledWith({ ...DEFAULT_SETTINGS, ffmpegPath: "/home/user/ffmpeg" }),
      );
      expect(screen.getByTestId("ffmpeg-path-input")).toHaveValue("/home/user/ffmpeg");
    });

    it("does nothing when the file picker is cancelled", async () => {
      openFileDialogMock.mockResolvedValue(null);
      const user = userEvent.setup();
      renderScreen();

      await user.click(screen.getByTestId("pdfium-browse"));

      await waitFor(() => expect(openFileDialogMock).toHaveBeenCalled());
      expect(onUpdate).not.toHaveBeenCalled();
    });

    it("resets a configured pdfium path to the default", async () => {
      const user = userEvent.setup();
      renderScreen({ ...DEFAULT_SETTINGS, pdfiumPath: "/opt/pdfium/libpdfium.so" });

      expect(screen.getByTestId("pdfium-path-input")).toHaveValue("/opt/pdfium/libpdfium.so");
      await user.click(screen.getByTestId("pdfium-reset"));

      expect(onUpdate).toHaveBeenCalledWith({ ...DEFAULT_SETTINGS, pdfiumPath: null });
    });

    it("does not show a reset button when no override is configured", () => {
      renderScreen();
      expect(screen.queryByTestId("ffmpeg-reset")).not.toBeInTheDocument();
      expect(screen.queryByTestId("pdfium-reset")).not.toBeInTheDocument();
    });
  });
});
