import { useEffect, useState } from "react";
import type { MenuMode, WedgeOption } from "../types";
import { describeWedge, polarToCartesian, wedgeAngles } from "../lib/wedgeGeometry";
import "./WedgeMenu.css";

interface WedgeMenuProps {
  open: boolean;
  mode: MenuMode;
  options: WedgeOption[];
  onSelect: (option: WedgeOption) => void;
  onCancel: () => void;
  /** The source file's own format/category (e.g. "JPG"), shown dimmed in
   * the hub per DESIGN.md — it's never itself a selectable target. */
  hubLabel?: string;
  /** The user's configured tools-menu combination (e.g. "Shift+Alt"),
   * shown in the onboarding hint badge — see `comboLabel` in
   * `lib/modifiers.ts`. Defaults to "Alt" (Phase 0's hardcoded default's
   * distinguishing key) when not given. */
  toolsHint?: string;
  /** Advanced tools that are currently in Phase 0 fake mode, with the real
   * GUI deferred to Phase 3. Shown only when mode is "tools" to avoid clutter
   * the format menu with unrelated "Not implemented" notes. */
  unimplementedTools?: string;
}

const SIZE = 320;
const CENTER = SIZE / 2;
const OUTER_RADIUS = 150;
const INNER_RADIUS = 40;
const HUB_RADIUS = 36;
const LABEL_RADIUS = (OUTER_RADIUS + INNER_RADIUS) / 2;

/** Modifier-key hint badges are an onboarding aid, not a persistent HUD —
 * DESIGN.md calls for showing them "only the first few times a user
 * encounters a given trigger". Tracked per-install via localStorage. */
const HINT_STORAGE_KEY = "satsuma.formatMenuHintShownCount";
const HINT_MAX_SHOWS = 3;

function useShouldShowToolsHint(open: boolean, mode: MenuMode): boolean {
  const [show, setShow] = useState(false);

  useEffect(() => {
    if (!open || mode !== "formats") {
      return;
    }
    try {
      const raw = window.localStorage.getItem(HINT_STORAGE_KEY);
      const count = raw ? parseInt(raw, 10) : 0;
      if (Number.isNaN(count) || count >= HINT_MAX_SHOWS) {
        setShow(false);
        return;
      }
      setShow(true);
      window.localStorage.setItem(HINT_STORAGE_KEY, String(count + 1));
    } catch {
      // localStorage unavailable (privacy mode, some test environments) —
      // just skip the onboarding hint rather than crash.
      setShow(false);
    }
    // Only re-evaluate when the menu transitions open in formats mode.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  return open && mode === "formats" && show;
}

export function WedgeMenu({ open, mode, options, onSelect, onCancel, hubLabel, toolsHint = "Alt", unimplementedTools }: WedgeMenuProps) {
  const [selectedIndex, setSelectedIndex] = useState(0);
  const showToolsHint = useShouldShowToolsHint(open, mode);

  // Reset selection whenever the menu (re)opens or its options change.
  useEffect(() => {
    if (open) setSelectedIndex(0);
  }, [open, options]);

  useEffect(() => {
    if (!open || options.length === 0) return;

    function handleKeyDown(event: KeyboardEvent) {
      switch (event.key) {
        case "ArrowRight":
        case "ArrowDown":
          event.preventDefault();
          setSelectedIndex((index) => (index + 1) % options.length);
          break;
        case "ArrowLeft":
        case "ArrowUp":
          event.preventDefault();
          setSelectedIndex((index) => (index - 1 + options.length) % options.length);
          break;
        case "Enter":
          event.preventDefault();
          setSelectedIndex((index) => {
            onSelect(options[index]);
            return index;
          });
          break;
        case "Escape":
          event.preventDefault();
          onCancel();
          break;
        default:
          break;
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, options, onSelect, onCancel]);

  if (!open || options.length === 0) return null;

  const angles = wedgeAngles(options.length);
  const menuLabel = mode === "formats" ? "Convert to format" : "Advanced tools";
  const hoveredOption = options[selectedIndex];
  const liveLabel = mode === "formats" ? `Convert to ${hoveredOption.label}` : hoveredOption.label;

  return (
    <div className="wedge-menu-overlay" onClick={onCancel} data-testid="wedge-menu">
      <div className="wedge-menu" onClick={(event) => event.stopPropagation()}>
        <div className="wedge-menu__live-label" data-testid="wedge-live-label">
          {liveLabel}
        </div>

        <svg width={SIZE} height={SIZE} viewBox={`0 0 ${SIZE} ${SIZE}`} role="menu" aria-label={menuLabel}>
          {options.map((option, index) => {
            const { start, end } = angles[index];
            const isSelected = index === selectedIndex;
            const path = describeWedge(CENTER, CENTER, OUTER_RADIUS, INNER_RADIUS, start, end);
            const labelPos = polarToCartesian(CENTER, CENTER, LABEL_RADIUS, (start + end) / 2);

            return (
              <g key={option.id}>
                <path
                  d={path}
                  className={`wedge${isSelected ? " wedge--selected" : ""}`}
                  role="menuitemradio"
                  aria-checked={isSelected}
                  data-testid={`wedge-${option.id}`}
                  onMouseEnter={() => setSelectedIndex(index)}
                  onClick={() => onSelect(option)}
                />
                <text
                  x={labelPos.x}
                  y={labelPos.y}
                  className="wedge-label"
                  textAnchor="middle"
                  pointerEvents="none"
                >
                  {option.icon} {option.label}
                </text>
              </g>
            );
          })}

          <circle cx={CENTER} cy={CENTER} r={HUB_RADIUS} className="wedge-hub" pointerEvents="none" />
          {hubLabel && (
            <text
              x={CENTER}
              y={CENTER}
              className="wedge-hub__label"
              textAnchor="middle"
              dominantBaseline="central"
              pointerEvents="none"
              data-testid="wedge-hub-label"
            >
              {hubLabel}
            </text>
          )}
        </svg>

        {/* Phase 3 future tools are not yet implemented in the UI, only a stub entry shown here. */}
        {mode === "tools" && unimplementedTools && (
          <div className="unimplemented-tools-note">
            <span className="unimplemented-tools-note__label">Not implemented yet:</span>
            <span className="unimplemented-tools-note__text">{unimplementedTools}</span>
          </div>
        )}

        {showToolsHint && (
          <div className="modifier-hint-badge" data-testid="modifier-hint-badge">
            <span className="modifier-hint-badge__key">{toolsHint}</span>
            for advanced tools
          </div>
        )}
      </div>
    </div>
  );
}
