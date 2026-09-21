import { useFileDrop } from "../hooks/useFileDrop";
import type { DroppedFile } from "../types";
import "./DropZone.css";

interface DropZoneProps {
  droppedFile: DroppedFile | null;
  onFileDropped: (file: DroppedFile) => void;
}

export function DropZone({ droppedFile, onFileDropped }: DropZoneProps) {
  const { isDragOver } = useFileDrop(onFileDropped);

  return (
    <div
      className={`drop-zone${isDragOver ? " drop-zone--active" : ""}`}
      data-testid="drop-zone"
    >
      {droppedFile ? (
        <>
          <p className="drop-zone__file-name" data-testid="dropped-file-name">
            {droppedFile.name}
          </p>
          <p className="drop-zone__category" data-testid="dropped-file-category">
            Detected type: {droppedFile.category}
          </p>
          <p className="drop-zone__hint">
            Hold Shift to open the format menu (Enter also works). Add Alt for tools.
          </p>
        </>
      ) : (
        <p className="drop-zone__hint">Drop a file here to get started</p>
      )}
    </div>
  );
}
