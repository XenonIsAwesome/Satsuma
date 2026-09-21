//! Archive <-> archive conversion (every pairwise ZIP/TAR/GZIP/RAR
//! combination) plus Extract Archive.
//!
//! ## Design
//!
//! Every direction is implemented the same way: read the source archive
//! fully into a `Vec<Entry>` (an in-memory list of relative paths, a
//! directory/file flag, and raw bytes), then write that list out through
//! whichever target format's writer is requested. This keeps the four
//! readers and four writers independent of each other (no N*N special
//! cases) and means archive -> archive conversion never touches a
//! temporary directory on disk.
//!
//! ## GZIP is single-stream: the `.tar.gz` rule
//!
//! GZIP can only ever hold one compressed payload, so:
//!
//! - Converting a ZIP/TAR/RAR archive *to* GZIP actually produces a
//!   `.tar.gz`: the archive's contents are bundled into a TAR stream
//!   first, then that whole TAR stream is gzipped as one payload (the
//!   standard "tarball" convention). Because `naming::output_path`
//!   doesn't know about this double-extension convention -- given a
//!   `"gz"` target extension it always computes `"<stem>.gz"` -- this
//!   module does **not** write to the literal `destination` path it is
//!   handed for a GZIP target. Instead [`convert`] recomputes a fresh,
//!   independently collision-checked `"<stem>.tar.gz"` path itself (via
//!   `naming::output_path(source, "tar.gz")`) and writes there, returning
//!   that real path rather than the nominal `destination` -- so the
//!   top-level `convert()` dispatcher in `mod.rs` reports the file that
//!   actually exists, and the toast after a ZIP/TAR/RAR -> GZIP conversion
//!   names `<stem>.tar.gz`, never a phantom `<stem>.gz`. This mirrors how
//!   the `document` family's multi-page PDF -> JPG/PNG export also writes
//!   into a path it computes itself rather than the literal `destination`
//!   argument (see `convert::mod`'s doc comment).
//! - A **bare `.gz`** source (a single gzipped file, not a tarball) is
//!   unwrapped to its one payload first, then repackaged as needed. A
//!   `.tar.gz`/`.tgz` source (a gzipped *tarball*) is a different case:
//!   both spell the same underlying `FileCategory::Archive` extension
//!   bucket (`extension_of` only ever sees the last dot-suffix, so
//!   `"backup.tar.gz"` and `"backup.tgz"` both report extension `"gz"`
//!   or `"tgz"` respectively), so this module tells them apart at
//!   *read* time, not from the extension: gunzip fully into memory, then
//!   try to parse the decompressed bytes as a TAR stream. If that
//!   succeeds, treat it as a multi-file tarball; otherwise treat the
//!   decompressed bytes as one opaque payload file (named from the GZIP
//!   header's stored original filename if present, else the source's
//!   file stem).
//!
//! ## The `.tar.gz` double suffix also shows up in *output* naming
//!
//! `naming::output_path` strips only the last dot-suffix, so a gzipped
//! tarball source would inherit a nonsense output stem: converting
//! `backup.tar.gz` to TAR would otherwise produce `backup.tar.tar` (and to
//! ZIP/RAR, `backup.tar.zip`/`backup.tar.rar`) — names that look like a
//! bug to users. [`convert`] therefore recomputes the destination from a
//! stem with *both* suffixes stripped whenever the source is a gzipped
//! tarball (`backup.tar.gz` -> `backup`, so `backup.zip`/`backup.tar`/
//! `backup.rar`), via `naming::unique_output_path`. Bare `.gz` sources
//! (and every non-GZIP source) keep the usual single-strip naming.
//!
//! ## Bounded decompression (zip-bomb / OOM backstop)
//!
//! Every reader materializes the archive's uncompressed contents in
//! memory, and a crafted archive can *claim* a huge payload while storing
//! only a few KB (`read_bounded`/`reject_oversized_entry`). Because
//! readers run inside `spawn_blocking` (see `src-tauri/src/lib.rs`), an
//! unbounded allocation there could abort the whole process — so reading
//! enforces three explicit budgets, converting an over-limit archive into
//! a [`ConvertError`] instead of an OOM:
//!
//! - `MAX_ENTRY_UNCOMPRESSED_BYTES` per entry (including a bare `.gz`
//!   payload; RAR rejects a too-large *declared* size from the file header
//!   before any of it is materialized, since `unrar` reads each entry as a
//!   single `Vec`);
//! - `MAX_TOTAL_UNCOMPRESSED_BYTES` summed across every entry of one
//!   archive (and across a `.tar.gz`'s whole decompressed stream);
//! - `MAX_ENTRY_COUNT` entries per archive.
//!
//! ## RAR reading: license judgment call
//!
//! There is no permissively-licensed Rust crate that reads *or* writes
//! RAR archives from scratch (RAR's compression algorithm is
//! proprietary). Reading uses the `unrar` crate, which compiles and
//! binds RARLAB's own official UnRAR C++ source
//! (`unrar_sys`/`vendor/unrar`) rather than reimplementing the format.
//! That source ships under its own (non-OSI, non-MIT/Apache) license,
//! reproduced here per that license's own paragraph 2 requirement to
//! include it "in source code comments of resulting package":
//!
//! > UnRAR source code may be used in any software to handle RAR
//! > archives without limitations free of charge, but cannot be used to
//! > develop RAR (WinRAR) compatible archiver and to re-create RAR
//! > compression algorithm, which is proprietary. Distribution of
//! > modified UnRAR source code in separate form or as a part of other
//! > software is permitted, provided that full text of this paragraph,
//! > starting from "UnRAR source code" words, is included in license, or
//! > in documentation if license is not available, and in source code
//! > comments of resulting package.
//!
//! Judgment call: this is used *exclusively* to read/extract RAR
//! archives -- never to write RAR compression -- which is precisely the
//! "without limitations free of charge" use case the license grants, and
//! the writing side of this module (see below) is an entirely
//! independent, from-scratch implementation that shares no code with
//! UnRAR. The license is still not MIT/Apache/BSD, is bundled as
//! compiled-in C++ source (not merely linked as a system library), and
//! its wording is unusual enough (Debian ships `unrar` in `non-free` on
//! account of it) that it warrants the project owner's explicit sign-off
//! before shipping a build that includes it, even though the use here
//! stays within the license's own explicit grant. See `Cargo.toml`'s
//! comment on the `unrar` dependency and this crate's report for the
//! same note.
//!
//! ## RAR writing: hand-rolled RAR5, store method only
//!
//! With no writer crate available at any license, and RAR's compression
//! algorithm out of reach (both for licensing and complexity reasons),
//! RAR output is a small hand-rolled RAR5 container writer implementing
//! only the "store" (uncompressed) method -- format-valid RAR5, just
//! with `compression method = 0`. This needs no compression algorithm at
//! all, only the container format: signature, vint encoding, block
//! headers (CRC32 + size + type + flags), a main archive header, one
//! file header per entry, and an end-of-archive marker. See
//! `rar5_writer` below for the implementation, cross-validated against
//! the system `unrar`/`7z` binaries in this module's tests (not just
//! read back by this module's own reader).

use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;

use super::error::ConvertError;
use super::naming::{output_dir_path, output_path};

// ---------------------------------------------------------------------
// Decompression budgets (see the module doc comment's "Bounded
// decompression" section)
// ---------------------------------------------------------------------

/// Upper bound on one entry's decompressed size. `Read::take`-truncated at
/// `limit + 1` so the rejection shows up as a [`ConvertError`] rather than
/// an in-memory zip-bomb allocation; RAR entries are rejected from their
/// header's declared size before any materialization. A few hundred MB is
/// far beyond any legitimate single file this app would archive.
const MAX_ENTRY_UNCOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;

/// Upper bound on the *total* decompressed size of an archive, summed
/// across every entry (and across a `.tar.gz`'s whole decompressed
/// stream). Prevents many individually-legal entries from combining into a
/// multi-GB RAM spike inside `spawn_blocking`.
const MAX_TOTAL_UNCOMPRESSED_BYTES: u64 = 512 * 1024 * 1024;

/// Upper bound on how many entries an archive may contain — a legitimate
/// archive has thousands of files, not a hundred thousand tiny ones
/// (the signature of an entry-count-bomb).
const MAX_ENTRY_COUNT: usize = 100_000;

/// Reads `reader` to the end but never materializes more than `limit`
/// bytes: the stream is truncated at `limit + 1` (via `Read::take`), and a
/// stream that still had bytes left is reported as a [`ConvertError`]
/// rather than silently truncated — turning an over-limit crafted entry
/// into a rejection instead of an unbounded allocation.
fn read_bounded(reader: impl Read, limit: u64, label: &str) -> Result<Vec<u8>, ConvertError> {
    let mut data = Vec::new();
    reader.take(limit + 1).read_to_end(&mut data)?;
    if data.len() as u64 > limit {
        return Err(ConvertError::Other(format!(
            "{label} decompresses to more than the {limit}-byte per-entry limit"
        )));
    }
    Ok(data)
}

/// Rejects an entry whose archive *header* declares more uncompressed
/// bytes than the per-entry budget, before any of them are materialized.
/// Used where the reader exposes the size ahead of reading (RAR's
/// `FileHeader::unpacked_size`); the streaming readers rely on
/// [`read_bounded`]'s take-truncation instead, which catches the same
/// condition as the data flows.
fn reject_oversized_entry(size: u64) -> Result<(), ConvertError> {
    if size > MAX_ENTRY_UNCOMPRESSED_BYTES {
        return Err(ConvertError::Other(format!(
            "archive entry claims {size} uncompressed bytes, exceeding the {}-byte per-entry limit",
            MAX_ENTRY_UNCOMPRESSED_BYTES
        )));
    }
    Ok(())
}

/// Accounts `size` materialized bytes against the archive-wide
/// [`MAX_TOTAL_UNCOMPRESSED_BYTES`] budget, rejecting the archive once the
/// running total passes it.
fn account_uncompressed(total: &mut u64, size: u64) -> Result<(), ConvertError> {
    *total += size;
    if *total > MAX_TOTAL_UNCOMPRESSED_BYTES {
        return Err(ConvertError::Other(format!(
            "archive decompresses to more than the {MAX_TOTAL_UNCOMPRESSED_BYTES}-byte total limit"
        )));
    }
    Ok(())
}

/// Appends `entry`, rejecting the archive once it holds
/// [`MAX_ENTRY_COUNT`] entries.
fn push_entry(entries: &mut Vec<Entry>, entry: Entry) -> Result<(), ConvertError> {
    if entries.len() >= MAX_ENTRY_COUNT {
        return Err(ConvertError::Other(format!(
            "archive contains more than {MAX_ENTRY_COUNT} entries"
        )));
    }
    entries.push(entry);
    Ok(())
}

/// The four archive formats this module knows about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Zip,
    Tar,
    Gzip,
    Rar,
}

/// Parses a *source* file's extension into a [`Format`]. Accepts `tgz` as
/// an alternate spelling of a gzipped tarball on input (see the module
/// doc comment); `tgz` is never produced as output.
fn parse_source_format(ext: &str) -> Option<Format> {
    match ext {
        "zip" => Some(Format::Zip),
        "tar" => Some(Format::Tar),
        "gz" | "tgz" => Some(Format::Gzip),
        "rar" => Some(Format::Rar),
        _ => None,
    }
}

/// Parses a requested *target* extension into a [`Format`]. Unlike
/// [`parse_source_format`], `tgz` is not accepted -- this module always
/// emits `gz` (as a `.tar.gz`) for the GZIP target, never `tgz`.
fn parse_target_format(ext: &str) -> Option<Format> {
    match ext {
        "zip" => Some(Format::Zip),
        "tar" => Some(Format::Tar),
        "gz" => Some(Format::Gzip),
        "rar" => Some(Format::Rar),
        _ => None,
    }
}

/// Extensions this module can produce from `source_ext`, excluding
/// `source_ext` itself.
pub fn supported_targets(source_ext: &str) -> &'static [&'static str] {
    match parse_source_format(&source_ext.to_lowercase()) {
        Some(Format::Zip) => &["tar", "gz", "rar"],
        Some(Format::Tar) => &["zip", "gz", "rar"],
        Some(Format::Gzip) => &["zip", "tar", "rar"],
        Some(Format::Rar) => &["zip", "tar", "gz"],
        None => &[],
    }
}

/// The output-name stem for a gzipped-tarball source (`.tar.gz`/`.tar.tgz`):
/// its file stem with the trailing `.tar` stripped (`backup.tar.gz` ->
/// `backup`), or `None` when the source isn't one — see the module doc
/// comment's "double suffix" section.
fn gzipped_tarball_stem(source: &Path) -> Option<String> {
    let stem = source.file_stem()?.to_str()?;
    if stem.to_lowercase().ends_with(".tar") {
        Some(stem[..stem.len() - 4].to_string())
    } else {
        None
    }
}

/// Converts `source` to `target_extension`, writing to `destination` and
/// returning the path actually written.
///
/// Every target except `gz` is written exactly to `destination` (and that
/// path is returned as-is) — *except* that a gzipped-tarball source
/// (.tar.gz/.tar.tgz) has its `destination` recomputed from a stem with
/// both suffixes stripped, so `backup.tar.gz` yields `backup.zip`/`
/// backup.tar`/`backup.rar` rather than the nonsense `backup.tar.<ext>`
/// (see `gzipped_tarball_stem`). A `gz` target is written to a path this
/// function computes itself instead (`"<source stem>.tar.gz"`) and *that*
/// real path is what's returned -- see the module doc comment's "GZIP is
/// single-stream" section for why the nominal `destination` can't be used
/// for it.
pub fn convert(source: &Path, target_extension: &str, destination: &Path) -> Result<PathBuf, ConvertError> {
    let source_ext = source_extension(source)?;
    let source_format = parse_source_format(&source_ext).ok_or_else(|| {
        ConvertError::InvalidSource(format!("unrecognized archive extension: .{source_ext}"))
    })?;
    let target_ext = target_extension.to_lowercase();
    let target_format = parse_target_format(&target_ext).ok_or_else(|| ConvertError::UnsupportedConversion {
        from: source_ext.clone(),
        to: target_ext.clone(),
    })?;
    if target_format == source_format {
        return Err(ConvertError::UnsupportedConversion {
            from: source_ext,
            to: target_ext,
        });
    }

    let entries = read_entries(source, source_format)?;

    let destination = match (source_format, gzipped_tarball_stem(source)) {
        (Format::Gzip, Some(stem)) => super::naming::unique_output_path(
            source.parent().unwrap_or_else(|| Path::new("")),
            &stem,
            &target_ext,
        ),
        _ => destination.to_path_buf(),
    };

    match target_format {
        Format::Zip => write_zip(&entries, &destination).map(|()| destination),
        Format::Tar => write_tar_file(&entries, &destination).map(|()| destination),
        Format::Gzip => write_tar_gz(&entries, source),
        Format::Rar => rar5_writer::write(&entries, &destination).map(|()| destination),
    }
}

/// Extracts `source` (a ZIP/TAR/GZIP/RAR archive) into a sibling folder
/// named after its stem (collision-safe, via `naming::output_dir_path`),
/// returning the folder's path. This is "Extract Archive" from Tools.md --
/// a single action with no settings screen, so it lives here rather than
/// behind Phase 4's dedicated-tool-GUI cutoff.
pub fn extract(source: &Path) -> Result<PathBuf, ConvertError> {
    let source_ext = source_extension(source)?;
    let source_format = parse_source_format(&source_ext).ok_or_else(|| {
        ConvertError::InvalidSource(format!("unrecognized archive extension: .{source_ext}"))
    })?;
    let entries = read_entries(source, source_format)?;

    let destination = output_dir_path(source, "Extracted");
    // Extract into a hidden sibling *staging* directory and rename it into
    // place only on success. A mid-extraction failure (disk full, or a
    // crafted entry like a file then `a/b` beneath it) otherwise leaves a
    // partial "Extracted" folder behind; with staging, the partial tree is
    // removed and the destination never appears at all. Staging lives in
    // the same parent directory as `destination` so the final `rename` is
    // same-filesystem and atomic-ish, never a copy across mounts.
    let staging = unique_staging_dir(&destination)?;
    let write_result = (|| -> Result<(), ConvertError> {
        std::fs::create_dir_all(&staging)?;
        for entry in &entries {
            let target = staging.join(&entry.path);
            if entry.is_dir {
                std::fs::create_dir_all(&target)?;
            } else {
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&target, &entry.data)?;
            }
        }
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    if let Err(error) = std::fs::rename(&staging, &destination) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(ConvertError::Io(error));
    }
    Ok(destination)
}

/// A not-yet-existing staging path inside `destination`'s parent directory
/// (so the final rename stays on one filesystem), dot-prefixed so it can
/// never be mistaken for extracted content.
fn unique_staging_dir(destination: &Path) -> Result<PathBuf, ConvertError> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let name = destination.file_name().and_then(|n| n.to_str()).unwrap_or("extract");
    for i in 0..10_000u32 {
        let candidate = parent.join(format!(".{name}.tmp-{i}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(ConvertError::Other(format!(
        "could not find a free staging directory next to {}",
        destination.display()
    )))
}

/// Returns the lowercased extension of `source`, or an `InvalidSource`
/// error if it has none.
fn source_extension(source: &Path) -> Result<String, ConvertError> {
    let source_str = source
        .to_str()
        .ok_or_else(|| ConvertError::InvalidSource("path is not valid UTF-8".to_string()))?;
    crate::file_type::extension_of(source_str)
        .ok_or_else(|| ConvertError::InvalidSource("source file has no extension".to_string()))
}

/// One entry read out of a source archive: a directory marker or a file
/// with its raw (already-decompressed) bytes.
struct Entry {
    /// Forward-slash-separated relative path inside the archive, already
    /// sanitized against `..`/absolute-path traversal (see
    /// [`sanitize_path`]).
    path: String,
    /// Whether this entry is a directory (in which case `data` is empty).
    is_dir: bool,
    /// The entry's raw, uncompressed bytes (empty for directories).
    data: Vec<u8>,
}

/// Reads every entry out of `source`, dispatching on its already-detected
/// [`Format`]. Results are then checked for duplicate sanitized paths (see
/// [`ensure_unique_paths`]) before anything is written.
fn read_entries(source: &Path, format: Format) -> Result<Vec<Entry>, ConvertError> {
    let entries = match format {
        Format::Zip => read_zip_entries(source),
        Format::Tar => {
            let file = File::open(source)?;
            read_tar_entries(io::BufReader::new(file))
        }
        Format::Gzip => read_gzip_entries(source),
        Format::Rar => read_rar_entries(source),
    }?;
    ensure_unique_paths(&entries)?;
    Ok(entries)
}

/// Rejects two entries whose independently-sanitized paths collide — e.g.
/// `folder/../README.md` and `README.md`, a file and a directory both
/// claiming `README.md`, or `Readme.md` and `readme.md`. Writing the second
/// would silently overwrite the first (in the in-memory list and in the
/// "Extracted" folder), and a colliding pair is the signature of a crafted
/// archive — legitimate ones don't contain duplicates — so the whole archive
/// is rejected up front, the same stance as the drive-letter rejection in
/// [`sanitize_path`]. Comparison folds ASCII case: NTFS (Windows' default
/// filesystem) treats path components case-insensitively, so a
/// case-sensitive check would wave `Readme.md`/`readme.md` through and the
/// second `fs::write` would then overwrite the first on the primary Windows
/// target — exactly the overwrite this check exists to prevent. ASCII-only
/// folding is enough: entry-name bytes pass through [`sanitize_path`]
/// verbatim, and NTFS case-insensitivity is itself ASCII-based.
/// Unconditional (not `#[cfg(windows)]`) because a ZIP built on Windows may
/// be extracted anywhere, and rejecting a case-colliding pair costs nothing
/// on Linux — the check only ever rejects crafted archives anyway.
fn ensure_unique_paths(entries: &[Entry]) -> Result<(), ConvertError> {
    let mut seen: HashSet<String> = HashSet::with_capacity(entries.len());
    for entry in entries {
        if !seen.insert(entry.path.to_ascii_lowercase()) {
            return Err(ConvertError::Other(format!(
                "refusing archive with duplicate entry path {entry_path:?} (two entries sanitize to the same name)",
                entry_path = entry.path
            )));
        }
    }
    Ok(())
}

/// Strips any `..`/`.`/empty path components and normalizes separators to
/// `/`, so a malicious or malformed archive entry can never escape the
/// extraction directory or the in-memory entry list via path traversal.
/// Also rejects any entry whose path contains a Windows drive-letter
/// component (`C:`/`c:`): on Windows `destination.join("C:/evil/...")`
/// treats the joined path as absolute and replaces the base path entirely,
/// writing outside the "Extracted" folder — the one absolute-path vector
/// that survives `..`/`.` filtering. The whole entry is rejected (rather
/// than silently renamed) since a drive-letter prefix in a downloaded
/// archive is exactly the crafted-archive attack this sanitizer exists for.
fn sanitize_path(raw: &str) -> Result<String, ConvertError> {
    let cleaned: Vec<&str> = raw
        .split(['/', '\\'])
        .filter(|part| !part.is_empty() && *part != "." && *part != "..")
        .collect();
    for part in &cleaned {
        if part.len() == 2 && part.as_bytes()[0].is_ascii_alphabetic() && part.as_bytes()[1] == b':' {
            return Err(ConvertError::Other(format!(
                "refusing archive entry with a drive-letter path: {raw:?}"
            )));
        }
    }
    if cleaned.is_empty() {
        Ok("_".to_string())
    } else {
        Ok(cleaned.join("/"))
    }
}

fn current_unix_time() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------
// ZIP
// ---------------------------------------------------------------------

fn read_zip_entries(source: &Path) -> Result<Vec<Entry>, ConvertError> {
    let file = File::open(source)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| ConvertError::Decode(format!("not a valid ZIP file: {e}")))?;
    let mut entries = Vec::with_capacity(archive.len().min(MAX_ENTRY_COUNT));
    let mut total_bytes: u64 = 0;
    for i in 0..archive.len() {
        let mut zip_entry = archive
            .by_index(i)
            .map_err(|e| ConvertError::Decode(format!("corrupt ZIP entry: {e}")))?;
        let is_dir = zip_entry.is_dir();
        let path = sanitize_path(zip_entry.name())?;
        let data = if is_dir {
            Vec::new()
        } else {
            let data = read_bounded(&mut zip_entry, MAX_ENTRY_UNCOMPRESSED_BYTES, "ZIP entry")?;
            account_uncompressed(&mut total_bytes, data.len() as u64)?;
            data
        };
        push_entry(&mut entries, Entry { path, is_dir, data })?;
    }
    Ok(entries)
}

fn write_zip(entries: &[Entry], destination: &Path) -> Result<(), ConvertError> {
    let file = File::create(destination)?;
    let mut writer = zip::ZipWriter::new(file);
    let file_options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let dir_options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for entry in entries {
        if entry.is_dir {
            writer
                .add_directory(entry.path.clone(), dir_options)
                .map_err(|e| ConvertError::Other(format!("could not write ZIP directory entry: {e}")))?;
        } else {
            writer
                .start_file(entry.path.clone(), file_options)
                .map_err(|e| ConvertError::Other(format!("could not write ZIP file entry: {e}")))?;
            writer.write_all(&entry.data)?;
        }
    }

    writer
        .finish()
        .map_err(|e| ConvertError::Other(format!("could not finalize ZIP archive: {e}")))?;
    Ok(())
}

// ---------------------------------------------------------------------
// TAR
// ---------------------------------------------------------------------

fn read_tar_entries<R: Read>(reader: R) -> Result<Vec<Entry>, ConvertError> {
    let mut archive = tar::Archive::new(reader);
    let mut entries = Vec::new();
    let mut total_bytes: u64 = 0;
    let raw_entries = archive.entries()?;
    for entry in raw_entries {
        let mut entry = entry.map_err(|e| ConvertError::Decode(format!("corrupt TAR entry: {e}")))?;
        let is_dir = entry.header().entry_type().is_dir();
        let raw_path = entry
            .path()
            .map_err(|e| ConvertError::Decode(format!("invalid TAR entry name: {e}")))?
            .to_string_lossy()
            .into_owned();
        let path = sanitize_path(&raw_path)?;
        let data = if is_dir {
            Vec::new()
        } else {
            let data = read_bounded(&mut entry, MAX_ENTRY_UNCOMPRESSED_BYTES, "TAR entry")?;
            account_uncompressed(&mut total_bytes, data.len() as u64)?;
            data
        };
        push_entry(&mut entries, Entry { path, is_dir, data })?;
    }
    Ok(entries)
}

/// Writes `entries` as a TAR stream into `writer`, returning the writer
/// (with the two required end-of-archive zero blocks already flushed) so
/// the caller can either close it directly (`.tar`) or feed it into a
/// GZIP encoder and finish that instead (`.tar.gz`).
fn write_tar_entries<W: Write>(entries: &[Entry], writer: W) -> io::Result<W> {
    let mut builder = tar::Builder::new(writer);
    let mtime = current_unix_time() as u64;

    for entry in entries {
        let mut header = tar::Header::new_gnu();
        header.set_mtime(mtime);
        if entry.is_dir {
            header.set_entry_type(tar::EntryType::Directory);
            header.set_mode(0o755);
            header.set_size(0);
            let mut name = entry.path.clone();
            if !name.ends_with('/') {
                name.push('/');
            }
            header.set_path(&name)?;
            header.set_cksum();
            builder.append(&header, io::empty())?;
        } else {
            header.set_entry_type(tar::EntryType::Regular);
            header.set_mode(0o644);
            header.set_size(entry.data.len() as u64);
            header.set_path(&entry.path)?;
            header.set_cksum();
            builder.append(&header, entry.data.as_slice())?;
        }
    }

    builder.into_inner()
}

fn write_tar_file(entries: &[Entry], destination: &Path) -> Result<(), ConvertError> {
    let file = File::create(destination)?;
    write_tar_entries(entries, file)?;
    Ok(())
}

// ---------------------------------------------------------------------
// GZIP (bare single-file .gz vs. .tar.gz tarball)
// ---------------------------------------------------------------------

/// Reads a `.gz`/`.tgz` source. Disambiguates a gzipped *tarball* (a
/// `.tar.gz`, holding several files) from a bare gzipped single file by
/// trying to parse the decompressed bytes as a TAR stream first -- see
/// the module doc comment.
fn read_gzip_entries(source: &Path) -> Result<Vec<Entry>, ConvertError> {
    let mut decoder = GzDecoder::new(File::open(source)?);
    let original_name = decoder
        .header()
        .and_then(|h| h.filename())
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned());

    // Bound the whole decompressed stream by the archive-wide total
    // (a `.tar.gz` holds many entries, whose own per-entry budgets apply
    // downstream in read_tar_entries). A GZIP *decode* failure is wrapped
    // as such; an over-limit rejection passes through untouched.
    let buffer = read_bounded(&mut decoder, MAX_TOTAL_UNCOMPRESSED_BYTES, "GZIP stream").map_err(|error| match error {
        ConvertError::Io(io_error) => ConvertError::Decode(format!("not a valid GZIP file: {io_error}")),
        other => other,
    })?;

    if looks_like_tar(&buffer) {
        return read_tar_entries(Cursor::new(buffer));
    }

    // A bare `.gz` payload is a single entry, so it also gets the
    // per-entry budget (the stream itself was bounded above by the
    // archive-wide total, which is looser).
    reject_oversized_entry(buffer.len() as u64)?;

    let name = original_name.unwrap_or_else(|| {
        source
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("payload")
            .to_string()
    });
    Ok(vec![Entry {
        path: sanitize_path(&name)?,
        is_dir: false,
        data: buffer,
    }])
}

/// Best-effort check for whether `buffer` is a valid TAR stream: TAR
/// entries are always 512-byte aligned with a checksummed header, so a
/// buffer that isn't actually TAR will essentially never parse cleanly.
fn looks_like_tar(buffer: &[u8]) -> bool {
    if buffer.len() < 512 || !buffer.len().is_multiple_of(512) {
        return false;
    }
    let mut archive = tar::Archive::new(Cursor::new(buffer));
    let raw_entries = match archive.entries() {
        Ok(entries) => entries,
        Err(_) => return false,
    };
    let mut saw_entry = false;
    for entry in raw_entries {
        if entry.is_err() {
            return false;
        }
        saw_entry = true;
    }
    saw_entry
}

/// Bundles `entries` into a TAR stream and gzips that stream as a single
/// payload, writing to a path computed from `source` (`"<stem>.tar.gz"`)
/// rather than the family-wide `output_path(source, "gz")` convention --
/// see the module doc comment's "GZIP is single-stream" section. Returns
/// the path actually written, since it differs from the nominal
/// `destination` handed to [`convert`].
fn write_tar_gz(entries: &[Entry], source: &Path) -> Result<PathBuf, ConvertError> {
    let destination = output_path(source, "tar.gz");
    let file = File::create(&destination)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let encoder = write_tar_entries(entries, encoder)?;
    encoder.finish()?;
    Ok(destination)
}

// ---------------------------------------------------------------------
// RAR reading (via the `unrar` crate -- see module doc comment)
// ---------------------------------------------------------------------

fn read_rar_entries(source: &Path) -> Result<Vec<Entry>, ConvertError> {
    let probe = unrar::Archive::new(source);
    if probe.is_multipart() {
        return Err(ConvertError::Other(
            "multivolume RAR archives (.partN.rar / .rNN) are not supported".to_string(),
        ));
    }

    let mut cursor = probe.open_for_processing().map_err(map_unrar_error)?;

    if cursor.has_encrypted_headers() {
        return Err(ConvertError::Other(
            "password-protected RAR archives are not supported".to_string(),
        ));
    }
    if cursor.volume_info() != unrar::VolumeInfo::None {
        return Err(ConvertError::Other(
            "multivolume RAR archives are not supported".to_string(),
        ));
    }

    let mut entries = Vec::new();
    let mut total_bytes: u64 = 0;
    loop {
        let with_header = match cursor.read_header().map_err(map_unrar_error)? {
            Some(with_header) => with_header,
            None => break,
        };

        let header = with_header.entry();
        if header.is_encrypted() {
            return Err(ConvertError::Other(
                "password-protected RAR archives are not supported".to_string(),
            ));
        }
        let is_dir = header.is_directory();
        let path = sanitize_path(&header.filename.to_string_lossy())?;

        if is_dir {
            cursor = with_header.skip().map_err(map_unrar_error)?;
            push_entry(&mut entries, Entry {
                path,
                is_dir: true,
                data: Vec::new(),
            })?;
        } else {
            // Reject an over-budget entry from its *declared* size before
            // materializing anything — `unrar`'s `read()` returns the whole
            // entry as a single Vec, so the header's `unpacked_size` is the
            // only place an over-limit entry can be stopped without first
            // allocating it (see the module doc comment's "Bounded
            // decompression" section).
            reject_oversized_entry(header.unpacked_size)?;
            let (data, next) = with_header.read().map_err(map_unrar_error)?;
            account_uncompressed(&mut total_bytes, data.len() as u64)?;
            push_entry(&mut entries, Entry {
                path,
                is_dir: false,
                data,
            })?;
            cursor = next;
        }
    }

    Ok(entries)
}

fn map_unrar_error(error: unrar::error::UnrarError) -> ConvertError {
    use unrar::error::Code;
    match error.code {
        Code::MissingPassword | Code::BadPassword => {
            ConvertError::Other("password-protected RAR archives are not supported".to_string())
        }
        _ => ConvertError::Decode(format!("RAR error: {error}")),
    }
}

// ---------------------------------------------------------------------
// RAR writing: hand-rolled RAR5 container, store method only
// ---------------------------------------------------------------------

/// A minimal RAR5 writer implementing only the "store" (uncompressed)
/// compression method, so no compression algorithm needs to be
/// implemented -- only the container format. Written against RARLAB's
/// publicly documented RAR 5.0 archive format (signature, vint encoding,
/// block header structure, main/file/end-of-archive header layouts).
///
/// Cross-validated in this module's tests against the system `unrar`
/// and `7z` binaries (`unrar t`/`unrar x`, `7z t`/`7z x`), not just read
/// back by this module's own [`read_rar_entries`].
mod rar5_writer {
    use super::{ConvertError, Entry};
    use std::path::Path;

    const SIGNATURE: [u8; 8] = [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00];

    const HEADER_TYPE_MAIN: u64 = 1;
    const HEADER_TYPE_FILE: u64 = 2;
    const HEADER_TYPE_END: u64 = 5;

    /// Common header flag: a data area (the entry's raw bytes) follows
    /// immediately after this header block.
    const HEADER_FLAG_DATA_AREA: u64 = 0x0002;

    /// File-header-specific flag: this entry is a directory.
    const FILE_FLAG_DIRECTORY: u64 = 0x0001;
    /// File-header-specific flag: the Unix-format `mtime` field is present.
    const FILE_FLAG_UNIX_TIME: u64 = 0x0002;
    /// File-header-specific flag: the `Data CRC32` field is present.
    const FILE_FLAG_HAS_CRC: u64 = 0x0004;

    /// Host OS value for "Unix" (as opposed to `0` for Windows).
    const HOST_OS_UNIX: u64 = 1;

    /// Writes `entries` as a valid, store-method RAR5 archive to `destination`.
    pub(super) fn write(entries: &[Entry], destination: &Path) -> Result<(), ConvertError> {
        let mtime = super::current_unix_time();
        let mut out = Vec::new();
        out.extend_from_slice(&SIGNATURE);

        // Main archive header: no volumes, no solid/lock/recovery flags.
        let mut main_body = Vec::new();
        write_vint(&mut main_body, 0); // archive flags
        out.extend_from_slice(&build_block(HEADER_TYPE_MAIN, 0, None, &main_body));

        for entry in entries {
            out.extend_from_slice(&file_block(entry, mtime));
        }

        // End-of-archive header: single volume, so end-of-archive flags = 0.
        let mut end_body = Vec::new();
        write_vint(&mut end_body, 0);
        out.extend_from_slice(&build_block(HEADER_TYPE_END, 0, None, &end_body));

        std::fs::write(destination, &out)?;
        Ok(())
    }

    /// Builds one file header block (plus its trailing data area, for a
    /// non-directory entry).
    fn file_block(entry: &Entry, mtime: u32) -> Vec<u8> {
        let mut file_flags = FILE_FLAG_UNIX_TIME;
        if entry.is_dir {
            file_flags |= FILE_FLAG_DIRECTORY;
        }
        let crc = if entry.is_dir {
            None
        } else {
            Some(crc32fast::hash(&entry.data))
        };
        if crc.is_some() {
            file_flags |= FILE_FLAG_HAS_CRC;
        }

        let mut body = Vec::new();
        write_vint(&mut body, file_flags);
        write_vint(&mut body, entry.data.len() as u64); // unpacked size
        let attributes: u64 = if entry.is_dir { 0o040_755 } else { 0o100_644 };
        write_vint(&mut body, attributes);
        body.extend_from_slice(&mtime.to_le_bytes());
        if let Some(crc) = crc {
            body.extend_from_slice(&crc.to_le_bytes());
        }
        // Compression information: method = 0 (store), algorithm version =
        // 0, solid = 0, dictionary size = 0 -- none of these matter for
        // the store method, so the whole vint is simply 0.
        write_vint(&mut body, 0);
        write_vint(&mut body, HOST_OS_UNIX);
        let name_bytes = entry.path.as_bytes();
        write_vint(&mut body, name_bytes.len() as u64);
        body.extend_from_slice(name_bytes);

        let header_flags = if entry.is_dir { 0 } else { HEADER_FLAG_DATA_AREA };
        let data_size = if entry.is_dir { None } else { Some(entry.data.len() as u64) };
        let mut block = build_block(HEADER_TYPE_FILE, header_flags, data_size, &body);
        if !entry.is_dir {
            block.extend_from_slice(&entry.data);
        }
        block
    }

    /// Builds one RAR5 block: `CRC32(4) || header_size(vint) || type(vint)
    /// || flags(vint) || [data_size(vint)] || type_specific`, per RAR5's
    /// documented header structure. The data area itself (if any) is
    /// *not* included here -- it is appended by the caller after this
    /// block, per spec (`Header size` only covers up to the end of the
    /// optional extra area, which this writer never emits).
    fn build_block(header_type: u64, header_flags: u64, data_size: Option<u64>, type_specific: &[u8]) -> Vec<u8> {
        let mut body = Vec::new();
        write_vint(&mut body, header_type);
        write_vint(&mut body, header_flags);
        if let Some(size) = data_size {
            write_vint(&mut body, size);
        }
        body.extend_from_slice(type_specific);

        let mut sized = Vec::new();
        write_vint(&mut sized, body.len() as u64);
        sized.extend_from_slice(&body);

        let crc = crc32fast::hash(&sized);
        let mut block = Vec::with_capacity(4 + sized.len());
        block.extend_from_slice(&crc.to_le_bytes());
        block.extend_from_slice(&sized);
        block
    }

    /// Encodes `value` as a RAR5 vint: 7 data bits per byte, high bit set
    /// on every byte except the last.
    fn write_vint(buf: &mut Vec<u8>, mut value: u64) {
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                buf.push(byte);
                break;
            }
            buf.push(byte | 0x80);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn vint_round_trips_small_and_large_values() {
            fn decode(buf: &[u8]) -> (u64, usize) {
                let mut value = 0u64;
                let mut shift = 0;
                for (i, &byte) in buf.iter().enumerate() {
                    value |= ((byte & 0x7f) as u64) << shift;
                    if byte & 0x80 == 0 {
                        return (value, i + 1);
                    }
                    shift += 7;
                }
                panic!("truncated vint");
            }

            for &value in &[0u64, 1, 127, 128, 300, 16384, u32::MAX as u64, u64::MAX] {
                let mut buf = Vec::new();
                write_vint(&mut buf, value);
                let (decoded, consumed) = decode(&buf);
                assert_eq!(decoded, value);
                assert_eq!(consumed, buf.len());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A fresh, empty temp directory for one test, cleaned up on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!("satsuma-archive-test-{}-{id}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    // -- fixture builders -------------------------------------------------

    /// The fixture content shared by every round-trip test: two text
    /// files at the top level plus one inside a subdirectory, so both
    /// file and directory entries get exercised.
    fn fixture_files() -> Vec<(&'static str, &'static [u8])> {
        vec![
            ("hello.txt", b"hello from satsuma\n" as &[u8]),
            ("notes.txt", b"line one\nline two\n"),
            ("sub/nested.txt", b"nested file contents\n"),
        ]
    }

    fn build_zip_fixture(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let file = File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        writer.add_directory("sub/", zip::write::SimpleFileOptions::default()).unwrap();
        for (name, data) in fixture_files() {
            writer.start_file(name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
        path
    }

    fn build_tar_fixture(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let file = File::create(&path).unwrap();
        let mut builder = tar::Builder::new(file);
        for (name, data) in fixture_files() {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append_data(&mut header, name, data).unwrap();
        }
        builder.finish().unwrap();
        path
    }

    fn build_targz_fixture(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let file = File::create(&path).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for (name, data) in fixture_files() {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append_data(&mut header, name, data).unwrap();
        }
        let encoder = builder.into_inner().unwrap();
        encoder.finish().unwrap();
        path
    }

    fn build_bare_gz_fixture(dir: &Path, name: &str, payload: &[u8]) -> PathBuf {
        let path = dir.join(name);
        let file = File::create(&path).unwrap();
        let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        encoder.write_all(payload).unwrap();
        encoder.finish().unwrap();
        path
    }

    fn read_file(path: &Path) -> Vec<u8> {
        std::fs::read(path).unwrap()
    }

    // -- external tool helpers --------------------------------------------

    fn run(cmd: &str, args: &[&str]) -> std::process::Output {
        std::process::Command::new(cmd)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("failed to run {cmd}: {e}"))
    }

    /// Prints the suite's standard "SKIP:" note and returns `true` when
    /// `tool` isn't on `PATH` — the real-tool RAR cross-validation below
    /// degrades to a clean skip on a machine without unrar/7z instead of
    /// failing, matching the audio.rs/video.rs convention for absent
    /// system tools. The RAR content checks that use this module's own
    /// reader always run regardless.
    fn tool_unavailable(tool: &str) -> bool {
        if which::which(tool).is_ok() {
            return false;
        }
        eprintln!("SKIP: {tool} not found on this system - skipping real-tool RAR cross-validation");
        true
    }

    fn assert_unrar_accepts(path: &Path) {
        let output = run("unrar", &["t", path.to_str().unwrap()]);
        assert!(
            output.status.success(),
            "unrar t failed on {}:\nstdout: {}\nstderr: {}",
            path.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn assert_7z_accepts(path: &Path) {
        let output = run("7z", &["t", path.to_str().unwrap()]);
        assert!(
            output.status.success(),
            "7z t failed on {}:\nstdout: {}\nstderr: {}",
            path.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // -- supported_targets --------------------------------------------------

    #[test]
    fn supported_targets_cover_every_pairwise_direction() {
        assert_eq!(supported_targets("zip"), &["tar", "gz", "rar"]);
        assert_eq!(supported_targets("tar"), &["zip", "gz", "rar"]);
        assert_eq!(supported_targets("gz"), &["zip", "tar", "rar"]);
        assert_eq!(supported_targets("tgz"), &["zip", "tar", "rar"]);
        assert_eq!(supported_targets("rar"), &["zip", "tar", "gz"]);
        assert_eq!(supported_targets("ZIP"), &["tar", "gz", "rar"]);
    }

    #[test]
    fn supported_targets_is_empty_for_unrelated_or_7z_extensions() {
        assert!(supported_targets("7z").is_empty());
        assert!(supported_targets("jpg").is_empty());
    }

    // -- zip <-> tar <-> gz <-> rar: all 12 pairwise directions --------------

    fn assert_zip_contains_fixture(path: &Path) {
        let file = File::open(path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        for (name, data) in fixture_files() {
            let mut entry = archive.by_name(name).unwrap_or_else(|_| panic!("missing {name} in {path:?}"));
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents).unwrap();
            assert_eq!(contents, data, "content mismatch for {name}");
        }
    }

    fn assert_tar_contains_fixture(reader: impl Read) {
        let mut archive = tar::Archive::new(reader);
        let mut found = std::collections::HashMap::new();
        for entry in archive.entries().unwrap() {
            let mut entry = entry.unwrap();
            let path = entry.path().unwrap().to_string_lossy().into_owned();
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents).unwrap();
            found.insert(path, contents);
        }
        for (name, data) in fixture_files() {
            assert_eq!(found.get(name).map(Vec::as_slice), Some(data), "content mismatch for {name}");
        }
    }

    fn assert_rar_contains_fixture(path: &Path) {
        let entries = read_rar_entries(path).unwrap();
        let mut found = std::collections::HashMap::new();
        for entry in entries {
            if !entry.is_dir {
                found.insert(entry.path, entry.data);
            }
        }
        for (name, data) in fixture_files() {
            assert_eq!(found.get(name).map(Vec::as_slice), Some(data), "content mismatch for {name}");
        }
        // Cross-validate independently of this module's own RAR reader,
        // but only when the system tools are present - skip cleanly
        // otherwise (see tool_unavailable).
        if !tool_unavailable("unrar") {
            assert_unrar_accepts(path);
        }
        if !tool_unavailable("7z") {
            assert_7z_accepts(path);
        }
    }

    #[test]
    fn zip_to_tar() {
        let dir = TempDir::new();
        let source = build_zip_fixture(dir.path(), "in.zip");
        let dest = dir.path().join("out.tar");
        convert(&source, "tar", &dest).unwrap();
        assert_tar_contains_fixture(File::open(&dest).unwrap());
    }

    #[test]
    fn zip_to_gz_produces_tar_gz() {
        let dir = TempDir::new();
        let source = build_zip_fixture(dir.path(), "in.zip");
        let dest = dir.path().join("in.gz"); // literal destination is ignored for gz
        let written = convert(&source, "gz", &dest).unwrap();
        assert!(!dest.exists(), "should not have written the literal .gz destination");
        let actual = dir.path().join("in.tar.gz");
        assert!(actual.exists(), "expected in.tar.gz to exist");
        assert_eq!(written, actual, "reported path must be the real .tar.gz, not the phantom .gz");
        let decoder = flate2::read::GzDecoder::new(File::open(&actual).unwrap());
        assert_tar_contains_fixture(decoder);
    }

    #[test]
    fn zip_to_rar() {
        let dir = TempDir::new();
        let source = build_zip_fixture(dir.path(), "in.zip");
        let dest = dir.path().join("out.rar");
        convert(&source, "rar", &dest).unwrap();
        assert_rar_contains_fixture(&dest);
    }

    #[test]
    fn tar_to_zip() {
        let dir = TempDir::new();
        let source = build_tar_fixture(dir.path(), "in.tar");
        let dest = dir.path().join("out.zip");
        convert(&source, "zip", &dest).unwrap();
        assert_zip_contains_fixture(&dest);
    }

    #[test]
    fn tar_to_gz_produces_tar_gz() {
        let dir = TempDir::new();
        let source = build_tar_fixture(dir.path(), "in.tar");
        let dest = dir.path().join("in.gz");
        let written = convert(&source, "gz", &dest).unwrap();
        let actual = dir.path().join("in.tar.gz");
        assert_eq!(written, actual, "reported path must be the real .tar.gz, not the phantom .gz");
        let decoder = flate2::read::GzDecoder::new(File::open(&actual).unwrap());
        assert_tar_contains_fixture(decoder);
    }

    #[test]
    fn tar_to_rar() {
        let dir = TempDir::new();
        let source = build_tar_fixture(dir.path(), "in.tar");
        let dest = dir.path().join("out.rar");
        convert(&source, "rar", &dest).unwrap();
        assert_rar_contains_fixture(&dest);
    }

    #[test]
    fn targz_to_zip() {
        let dir = TempDir::new();
        let source = build_targz_fixture(dir.path(), "backup.tar.gz");
        // A gzipped-tarball source gets both suffixes stripped for its
        // output name (backup.tar.gz -> backup.zip), so the literal
        // destination passed below is recomputed.
        let dest = dir.path().join("out.zip");
        let written = convert(&source, "zip", &dest).unwrap();
        let actual = dir.path().join("backup.zip");
        assert!(actual.exists(), "expected backup.zip, not the single-stripped backup.tar.zip");
        assert_eq!(written, actual);
        assert_zip_contains_fixture(&actual);
    }

    #[test]
    fn targz_to_tar() {
        let dir = TempDir::new();
        let source = build_targz_fixture(dir.path(), "backup.tar.gz");
        let dest = dir.path().join("out.tar");
        let written = convert(&source, "tar", &dest).unwrap();
        let actual = dir.path().join("backup.tar");
        assert_eq!(written, actual, "backup.tar.gz -> tar must yield backup.tar, not backup.tar.tar");
        assert_tar_contains_fixture(File::open(&actual).unwrap());
    }

    #[test]
    fn targz_to_rar() {
        let dir = TempDir::new();
        let source = build_targz_fixture(dir.path(), "backup.tar.gz");
        let dest = dir.path().join("out.rar");
        let written = convert(&source, "rar", &dest).unwrap();
        let actual = dir.path().join("backup.rar");
        assert_eq!(written, actual, "backup.tar.gz -> rar must yield backup.rar, not backup.tar.rar");
        assert_rar_contains_fixture(&actual);
    }

    #[test]
    fn targz_to_zip_names_are_collision_safe_like_any_other_output() {
        let dir = TempDir::new();
        let source = build_targz_fixture(dir.path(), "backup.tar.gz");
        std::fs::write(dir.path().join("backup.zip"), b"already here").unwrap();
        let dest = dir.path().join("out.zip");
        let written = convert(&source, "zip", &dest).unwrap();
        let actual = dir.path().join("backup 2.zip");
        assert_eq!(written, actual);
        assert_zip_contains_fixture(&actual);
    }

    #[test]
    fn rar_to_zip() {
        let dir = TempDir::new();
        let entries: Vec<Entry> = fixture_files()
            .into_iter()
            .map(|(name, data)| Entry {
                path: name.to_string(),
                is_dir: false,
                data: data.to_vec(),
            })
            .collect();
        let source = dir.path().join("in.rar");
        rar5_writer::write(&entries, &source).unwrap();
        if !tool_unavailable("unrar") {
            assert_unrar_accepts(&source);
        }

        let dest = dir.path().join("out.zip");
        convert(&source, "zip", &dest).unwrap();
        assert_zip_contains_fixture(&dest);
    }

    #[test]
    fn rar_to_tar() {
        let dir = TempDir::new();
        let entries: Vec<Entry> = fixture_files()
            .into_iter()
            .map(|(name, data)| Entry {
                path: name.to_string(),
                is_dir: false,
                data: data.to_vec(),
            })
            .collect();
        let source = dir.path().join("in.rar");
        rar5_writer::write(&entries, &source).unwrap();

        let dest = dir.path().join("out.tar");
        convert(&source, "tar", &dest).unwrap();
        assert_tar_contains_fixture(File::open(&dest).unwrap());
    }

    #[test]
    fn rar_to_gz_produces_tar_gz() {
        let dir = TempDir::new();
        let entries: Vec<Entry> = fixture_files()
            .into_iter()
            .map(|(name, data)| Entry {
                path: name.to_string(),
                is_dir: false,
                data: data.to_vec(),
            })
            .collect();
        let source = dir.path().join("in.rar");
        rar5_writer::write(&entries, &source).unwrap();

        let dest = dir.path().join("in.gz");
        let written = convert(&source, "gz", &dest).unwrap();
        let actual = dir.path().join("in.tar.gz");
        assert_eq!(written, actual, "reported path must be the real .tar.gz, not the phantom .gz");
        let decoder = flate2::read::GzDecoder::new(File::open(&actual).unwrap());
        assert_tar_contains_fixture(decoder);
    }

    // -- RAR5 writer: directory entries + real-tool round trip --------------

    #[test]
    fn rar5_writer_round_trips_directories_and_is_valid_per_system_tools() {
        let dir = TempDir::new();
        let entries = vec![
            Entry {
                path: "sub".to_string(),
                is_dir: true,
                data: Vec::new(),
            },
            Entry {
                path: "sub/nested.txt".to_string(),
                is_dir: false,
                data: b"nested file contents\n".to_vec(),
            },
            Entry {
                path: "top.txt".to_string(),
                is_dir: false,
                data: b"top level file\n".to_vec(),
            },
        ];
        let path = dir.path().join("dirs.rar");
        rar5_writer::write(&entries, &path).unwrap();

        let unrar_ok = !tool_unavailable("unrar");
        let seven_ok = !tool_unavailable("7z");
        if unrar_ok {
            assert_unrar_accepts(&path);
        }
        if seven_ok {
            assert_7z_accepts(&path);
        }

        // Extract with the real `unrar` binary and check byte-for-byte
        // content, independent of this module's own RAR reader.
        if unrar_ok {
            let extract_dir = dir.path().join("extracted");
            std::fs::create_dir_all(&extract_dir).unwrap();
            let output = run(
                "unrar",
                &["x", "-y", path.to_str().unwrap(), &format!("{}/", extract_dir.display())],
            );
            assert!(output.status.success(), "unrar x failed: {}", String::from_utf8_lossy(&output.stderr));
            assert_eq!(read_file(&extract_dir.join("top.txt")), b"top level file\n");
            assert_eq!(read_file(&extract_dir.join("sub/nested.txt")), b"nested file contents\n");
        }
    }

    #[test]
    fn rar5_writer_handles_empty_file() {
        let dir = TempDir::new();
        let entries = vec![Entry {
            path: "empty.txt".to_string(),
            is_dir: false,
            data: Vec::new(),
        }];
        let path = dir.path().join("empty.rar");
        rar5_writer::write(&entries, &path).unwrap();
        if !tool_unavailable("unrar") {
            assert_unrar_accepts(&path);
        }

        let read_back = read_rar_entries(&path).unwrap();
        assert_eq!(read_back.len(), 1);
        assert_eq!(read_back[0].data, Vec::<u8>::new());
    }

    // -- bare .gz vs .tar.gz disambiguation ----------------------------------

    #[test]
    fn bare_gz_converts_to_zip_with_just_that_one_file() {
        let dir = TempDir::new();
        let source = build_bare_gz_fixture(dir.path(), "notes.txt.gz", b"just one file\n");
        let dest = dir.path().join("out.zip");
        convert(&source, "zip", &dest).unwrap();

        let file = File::open(&dest).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        assert_eq!(archive.len(), 1, "expected exactly one file in the zip, not a tarball's worth");
        let mut entry = archive.by_index(0).unwrap();
        assert_eq!(entry.name(), "notes.txt");
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents).unwrap();
        assert_eq!(contents, b"just one file\n");
    }

    #[test]
    fn bare_gz_falls_back_to_source_stem_without_gzip_filename_header() {
        // build_bare_gz_fixture always sets a GZIP filename header via
        // GzEncoder, so exercise the no-header fallback path directly.
        let dir = TempDir::new();
        let path = dir.path().join("mystery.gz");
        {
            let file = File::create(&path).unwrap();
            let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            encoder.write_all(b"opaque bytes").unwrap();
            encoder.finish().unwrap();
        }
        let entries = read_gzip_entries(&path).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].is_dir);
        assert_eq!(entries[0].data, b"opaque bytes");
    }

    #[test]
    fn tar_gz_tarball_is_not_treated_as_a_single_opaque_file() {
        let dir = TempDir::new();
        let source = build_targz_fixture(dir.path(), "bundle.tar.gz");
        let entries = read_gzip_entries(&source).unwrap();
        let file_count = entries.iter().filter(|e| !e.is_dir).count();
        assert_eq!(file_count, fixture_files().len(), "expected every fixture file to survive tar.gz detection");
    }

    // -- extract() ------------------------------------------------------------

    #[test]
    fn extract_zip_creates_collision_safe_sibling_folder_with_contents() {
        let dir = TempDir::new();
        let source = build_zip_fixture(dir.path(), "bundle.zip");
        let extracted = extract(&source).unwrap();
        assert_eq!(extracted, dir.path().join("bundle Extracted"));
        for (name, data) in fixture_files() {
            assert_eq!(read_file(&extracted.join(name)), data);
        }

        // A second extraction must not collide with the first.
        let extracted_again = extract(&source).unwrap();
        assert_eq!(extracted_again, dir.path().join("bundle Extracted 2"));
    }

    #[test]
    fn extract_tar() {
        let dir = TempDir::new();
        let source = build_tar_fixture(dir.path(), "bundle.tar");
        let extracted = extract(&source).unwrap();
        for (name, data) in fixture_files() {
            assert_eq!(read_file(&extracted.join(name)), data);
        }
    }

    #[test]
    fn extract_gzip_tarball() {
        let dir = TempDir::new();
        let source = build_targz_fixture(dir.path(), "bundle.tar.gz");
        let extracted = extract(&source).unwrap();
        for (name, data) in fixture_files() {
            assert_eq!(read_file(&extracted.join(name)), data);
        }
    }

    #[test]
    fn extract_bare_gzip() {
        let dir = TempDir::new();
        let source = build_bare_gz_fixture(dir.path(), "notes.txt.gz", b"solo payload\n");
        let extracted = extract(&source).unwrap();
        assert_eq!(read_file(&extracted.join("notes.txt")), b"solo payload\n");
    }

    #[test]
    fn extract_rar() {
        let dir = TempDir::new();
        let entries: Vec<Entry> = fixture_files()
            .into_iter()
            .map(|(name, data)| Entry {
                path: name.to_string(),
                is_dir: false,
                data: data.to_vec(),
            })
            .collect();
        let source = dir.path().join("bundle.rar");
        rar5_writer::write(&entries, &source).unwrap();

        let extracted = extract(&source).unwrap();
        for (name, data) in fixture_files() {
            assert_eq!(read_file(&extracted.join(name)), data);
        }
    }

    #[test]
    fn extract_failure_removes_the_partial_staging_tree_and_creates_no_folder() {
        let dir = TempDir::new();
        // A file entry `a` followed by `a/b` beneath it (impossible to
        // satisfy: `a` is already a file when the second entry needs to
        // make it a directory) fails extraction mid-write.
        let path = dir.path().join("crafted.zip");
        let file = File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("a", options).unwrap();
        writer.write_all(b"file a").unwrap();
        writer.start_file("a/b", options).unwrap();
        writer.write_all(b"file a/b").unwrap();
        writer.finish().unwrap();

        let result = extract(&path);
        assert!(result.is_err(), "extraction that collides a file with a directory must fail");
        let src_dir = dir.path().to_path_buf();
        let leftover_staging = std::fs::read_dir(&src_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with(".crafted Extracted.tmp"));
        assert!(!leftover_staging, "a failed extraction must leave no staging directory behind");
        assert!(!dir.path().join("crafted Extracted").exists(), "a failed extraction must never create the destination folder");
    }

    #[test]
    fn extract_rejects_two_entries_that_sanitize_to_the_same_path() {
        // `a/../b.txt` and `a/b.txt` are different raw archive paths but
        // both sanitize to `a/b.txt` — the second would silently overwrite
        // the first, the signature of a crafted archive. Rejected up front
        // (before any write), consistent with the drive-letter rejection.
        let dir = TempDir::new();
        let path = dir.path().join("colliding.zip");
        let file = File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("a/../b.txt", options).unwrap();
        writer.write_all(b"first").unwrap();
        writer.start_file("a/b.txt", options).unwrap();
        writer.write_all(b"second").unwrap();
        writer.finish().unwrap();

        let result = extract(&path);
        assert!(matches!(result, Err(ConvertError::Other(_))));
        assert!(!dir.path().join("colliding Extracted").exists());
    }

    #[test]
    fn extract_rejects_two_entries_whose_names_collide_case_insensitively() {
        // NTFS compares path components case-insensitively (Windows' default
        // filesystem), so `Readme.md` and `readme.md` collide on disk even
        // though they're distinct strings — a case-sensitive duplicate check
        // would wave this pair through and the second write would overwrite
        // the first on the primary Windows target. Rejected up front, like
        // the exact-match collision above.
        let dir = TempDir::new();
        let path = dir.path().join("case-colliding.zip");
        let file = File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("Readme.md", options).unwrap();
        writer.write_all(b"first").unwrap();
        writer.start_file("readme.md", options).unwrap();
        writer.write_all(b"second").unwrap();
        writer.finish().unwrap();

        let result = extract(&path);
        assert!(matches!(result, Err(ConvertError::Other(_))));
        assert!(!dir.path().join("case-colliding Extracted").exists());
    }

    #[test]
    fn convert_rejects_two_entries_that_sanitize_to_the_same_path() {
        let dir = TempDir::new();
        let path = dir.path().join("colliding.zip");
        let file = File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        // The same sanitization-collision pair as extract_rejects above
        // (`a/../b.txt` cleans to `a/b.txt`): distinct raw paths, one
        // destination entry — rejected at read time, before any write.
        writer.start_file("a/../b.txt", options).unwrap();
        writer.write_all(b"first").unwrap();
        writer.start_file("a/b.txt", options).unwrap();
        writer.write_all(b"second").unwrap();
        writer.finish().unwrap();

        let dest = dir.path().join("out.tar");
        let result = convert(&path, "tar", &dest);
        assert!(matches!(result, Err(ConvertError::Other(_))));
    }

    // -- rejections -------------------------------------------------------

    #[test]
    fn convert_rejects_unsupported_target_extension() {
        let dir = TempDir::new();
        let source = build_zip_fixture(dir.path(), "in.zip");
        let dest = dir.path().join("out.7z");
        let result = convert(&source, "7z", &dest);
        assert!(matches!(result, Err(ConvertError::UnsupportedConversion { .. })));
    }

    #[test]
    fn convert_rejects_converting_a_format_to_itself() {
        let dir = TempDir::new();
        let source = build_zip_fixture(dir.path(), "in.zip");
        let dest = dir.path().join("out.zip");
        let result = convert(&source, "zip", &dest);
        assert!(matches!(result, Err(ConvertError::UnsupportedConversion { .. })));
    }

    #[test]
    fn convert_rejects_tgz_as_a_target_extension() {
        let dir = TempDir::new();
        let source = build_zip_fixture(dir.path(), "in.zip");
        let dest = dir.path().join("out.tgz");
        let result = convert(&source, "tgz", &dest);
        assert!(matches!(result, Err(ConvertError::UnsupportedConversion { .. })));
    }

    #[test]
    fn seven_zip_extension_is_not_handled_by_this_module() {
        assert!(supported_targets("7z").is_empty());
        let dir = TempDir::new();
        let source = dir.path().join("archive.7z");
        std::fs::write(&source, b"not really a 7z file").unwrap();
        let dest = dir.path().join("out.zip");
        let result = convert(&source, "zip", &dest);
        assert!(matches!(result, Err(ConvertError::InvalidSource(_))));
    }

    /// Multivolume detection is purely filename-based and needs no
    /// archive I/O at all (see `unrar::Archive::is_multipart`), so it's
    /// tested directly rather than via a real multivolume fixture --
    /// this sandbox has no proprietary `rar` CLI available to create
    /// one, and 7z/unrar can't write RAR. Password-protection rejection
    /// (`read_rar_entries`'s `is_encrypted()`/`has_encrypted_headers()`
    /// checks) is implemented the same way but is *not* covered by an
    /// automated test for the same reason plus one more: `unrar`'s
    /// `FileHeader` type has private fields with no public constructor,
    /// so a fake encrypted header can't be built in-process either. See
    /// this crate's delivery report for the same caveat.
    #[test]
    fn multivolume_rar_filenames_are_detected_before_any_io() {
        assert!(unrar::Archive::new(Path::new("archive.part1.rar")).is_multipart());
        assert!(unrar::Archive::new(Path::new("archive.part42.rar")).is_multipart());
        assert!(unrar::Archive::new(Path::new("archive.r00")).is_multipart());
        assert!(unrar::Archive::new(Path::new("archive.r10")).is_multipart());
        assert!(!unrar::Archive::new(Path::new("archive.rar")).is_multipart());
        assert!(!unrar::Archive::new(Path::new("archive.zip")).is_multipart());
    }

    #[test]
    fn read_rar_entries_rejects_a_multipart_named_source_without_opening_it() {
        let dir = TempDir::new();
        // Deliberately not a real archive -- if detection required
        // opening the file this would fail with a decode error instead
        // of the expected multivolume rejection.
        let source = dir.path().join("archive.part1.rar");
        std::fs::write(&source, b"not a real rar file").unwrap();
        let result = read_rar_entries(&source);
        assert!(matches!(result, Err(ConvertError::Other(_))));
    }

    #[test]
    fn sanitize_path_strips_traversal_and_normalizes_separators() {
        assert_eq!(sanitize_path("a/b/c.txt").unwrap(), "a/b/c.txt");
        assert_eq!(sanitize_path("a\\b\\c.txt").unwrap(), "a/b/c.txt");
        assert_eq!(sanitize_path("../../etc/passwd").unwrap(), "etc/passwd");
        assert_eq!(sanitize_path("/etc/passwd").unwrap(), "etc/passwd");
        assert_eq!(sanitize_path("./a/./b").unwrap(), "a/b");
        assert_eq!(sanitize_path("../..").unwrap(), "_");
    }

    #[test]
    fn sanitize_path_rejects_windows_drive_letter_prefixes() {
        // Windows-only escape: `destination.join("C:/evil/payload.exe")`
        // treats the joined path as absolute and replaces the base path
        // entirely, letting a crafted archive write outside the "Extracted"
        // folder. `..` filtering alone never catches this, so the whole
        // entry must be rejected.
        assert!(sanitize_path("C:/evil/payload.exe").is_err());
        assert!(sanitize_path("c:\\evil\\payload.exe").is_err());
        assert!(sanitize_path("Z:/x.txt").is_err());
        // Drive prefix in a middle component is the same absolute-path
        // vector as at the start.
        assert!(sanitize_path("folder/C:/payload.exe").is_err());
        // A colon that isn't a drive prefix (e.g. a filename containing a
        // colon) is not confused for one, and normal relative paths pass.
        assert_eq!(sanitize_path("a:b.txt").unwrap(), "a:b.txt");
        assert_eq!(sanitize_path("normal/file.txt").unwrap(), "normal/file.txt");
    }

    // -- decompression budgets (zip-bomb backstop) --------------------------

    #[test]
    fn read_bounded_rejects_a_stream_longer_than_the_limit_and_passes_an_in_limit_one() {
        let over = read_bounded(io::Cursor::new(b"0123456789".to_vec()), 4, "test entry").unwrap_err();
        assert!(matches!(over, ConvertError::Other(_)));
        assert!(over.to_string().contains("4-byte per-entry limit"));
        let within = read_bounded(io::Cursor::new(b"0123".to_vec()), 4, "test entry").unwrap();
        assert_eq!(within, b"0123".to_vec());
    }

    #[test]
    fn reject_oversized_entry_rejects_a_declared_size_over_the_per_entry_limit() {
        assert!(reject_oversized_entry(MAX_ENTRY_UNCOMPRESSED_BYTES).is_ok());
        let over = reject_oversized_entry(MAX_ENTRY_UNCOMPRESSED_BYTES + 1).unwrap_err();
        assert!(matches!(over, ConvertError::Other(_)));
    }

    #[test]
    fn account_uncompressed_rejects_once_the_total_passes_the_archive_wide_budget() {
        let mut total: u64 = MAX_TOTAL_UNCOMPRESSED_BYTES - 1;
        account_uncompressed(&mut total, 1).unwrap();
        assert_eq!(total, MAX_TOTAL_UNCOMPRESSED_BYTES);
        let over = account_uncompressed(&mut total, 1).unwrap_err();
        assert!(matches!(over, ConvertError::Other(_)));
    }

    #[test]
    fn zip_with_more_entries_than_the_count_cap_is_rejected() {
        let dir = TempDir::new();
        let path = dir.path().join("many.zip");
        let file = File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        for i in 0..MAX_ENTRY_COUNT + 1 {
            writer.start_file(format!("f{i}.txt"), options).unwrap();
            writer.write_all(b"x").unwrap();
        }
        writer.finish().unwrap();

        let dest = dir.path().join("out.tar");
        let result = convert(&path, "tar", &dest);
        assert!(matches!(result, Err(ConvertError::Other(_))), "an archive over the entry count cap must be rejected");
        assert!(!dest.exists());
    }
}
