import { describe, expect, it } from "vitest";
import { basename, extensionOf } from "./path";

describe("basename", () => {
  it("extracts the file name from a unix path", () => {
    expect(basename("/home/user/photo.png")).toBe("photo.png");
  });

  it("extracts the file name from a windows path", () => {
    expect(basename("C:\\Users\\me\\clip.mp4")).toBe("clip.mp4");
  });

  it("returns the input unchanged when there is no separator", () => {
    expect(basename("song.mp3")).toBe("song.mp3");
  });
});

describe("extensionOf", () => {
  it("returns the lowercased extension", () => {
    expect(extensionOf("/a/b/IMAGE.PNG")).toBe("png");
  });

  it("returns null when there is no extension", () => {
    expect(extensionOf("README")).toBeNull();
  });

  it("returns null for dotfiles", () => {
    expect(extensionOf(".gitignore")).toBeNull();
  });

  it("works with windows-style paths", () => {
    expect(extensionOf("C:\\Users\\me\\clip.MOV")).toBe("mov");
  });
});
