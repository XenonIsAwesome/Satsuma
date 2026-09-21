//! Video <-> video conversion (MP4/MOV/MKV/WebM/AVI/WMV/GIF) plus MP3 audio
//! export, via a bundled FFmpeg sidecar binary. The binary's path is passed
//! in by the Tauri layer (see `src-tauri`'s sidecar resolution) rather than
//! looked up here, so this module stays Tauri-free and testable by pointing
//! it at any FFmpeg binary (e.g. the system one, in tests).
//!
//! `{mp4, mov, mkv, webm, avi, wmv}` are fully pairwise interconvertible and
//! each also gets `"mp3"` as an extra target (audio-only export, dropping
//! video). `"gif"` is a full member of the pairwise set too (source and
//! target), but has no audio track, so it never offers `"mp3"` and is
//! never itself offered as an export-audio target.

use super::error::ConvertError;
use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::{BufReader, Read};
use std::path::Path;

/// Extensions this module can produce from `source_ext`, excluding
/// `source_ext` itself.
pub fn supported_targets(source_ext: &str) -> &'static [&'static str] {
    match source_ext.to_lowercase().as_str() {
        "mp4" => &["mov", "mkv", "webm", "avi", "wmv", "gif", "mp3"],
        "mov" => &["mp4", "mkv", "webm", "avi", "wmv", "gif", "mp3"],
        "mkv" => &["mp4", "mov", "webm", "avi", "wmv", "gif", "mp3"],
        "webm" => &["mp4", "mov", "mkv", "avi", "wmv", "gif", "mp3"],
        "avi" => &["mp4", "mov", "mkv", "webm", "wmv", "gif", "mp3"],
        "wmv" => &["mp4", "mov", "mkv", "webm", "avi", "gif", "mp3"],
        "gif" => &["mp4", "mov", "mkv", "webm", "avi", "wmv"],
        _ => &[],
    }
}

/// Converts `source` to `target_extension` using the FFmpeg binary at
/// `ffmpeg_bin`, writing to `destination`. `on_progress` is called with a
/// `0.0..=1.0` fraction as FFmpeg reports encode timestamps against the
/// source's known duration (parsed from FFmpeg's own stderr output), and
/// at least once with something close to `1.0` on success.
pub fn convert(
    ffmpeg_bin: &Path,
    source: &Path,
    target_extension: &str,
    destination: &Path,
    on_progress: &mut dyn FnMut(f32),
) -> Result<(), ConvertError> {
    let target = target_extension.to_lowercase();
    let source_ext = source
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .ok_or_else(|| ConvertError::InvalidSource("source file has no extension".to_string()))?;

    if !supported_targets(&source_ext).contains(&target.as_str()) {
        return Err(ConvertError::UnsupportedConversion {
            from: source_ext,
            to: target,
        });
    }

    if !source.is_file() {
        return Err(ConvertError::InvalidSource(format!("not a file: {}", source.display())));
    }

    let args = ffmpeg_args(&target, source, destination);

    let mut child = super::build_ffmpeg_command(ffmpeg_bin)
        .args(&args)
        .spawn()
        .map_err(|error| ConvertError::EngineUnavailable(format!("failed to launch ffmpeg: {error}")))?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ConvertError::Other("failed to capture ffmpeg output".to_string()))?;
    let mut reader = BufReader::new(stderr);

    let mut duration_seconds: Option<f64> = None;
    let mut tail_lines: VecDeque<String> = VecDeque::with_capacity(40);

    stream_progress(&mut reader, &mut duration_seconds, &mut tail_lines, on_progress);

    let status = child
        .wait()
        .map_err(|error| ConvertError::Other(format!("failed waiting for ffmpeg to exit: {error}")))?;

    if status.success() && destination.is_file() {
        on_progress(1.0);
        return Ok(());
    }

    let tail = tail_lines.into_iter().collect::<Vec<_>>().join("\n");
    let detail = trim_tail(&tail, 500);
    Err(ConvertError::Other(format!(
        "ffmpeg could not convert {} to .{target}: {}",
        source.display(),
        if detail.is_empty() {
            "no output captured".to_string()
        } else {
            detail
        }
    )))
}

/// Reads FFmpeg's stderr to completion byte-wise, splitting on BOTH
/// `\n`-terminated log lines AND `\r`-terminated intermediate progress
/// lines — FFmpeg prints its running `frame=... time=...` stats ending in
/// `\r`, with only the final summary line ending in `\n` — so every
/// intermediate update is handled individually instead of being merged
/// into one line and reduced to its first timestamp. Parses `Duration:` /
/// `time=` to drive `on_progress` and keeps the last [`TAIL_LINES`]
/// complete lines for the error message. Mirrors `audio::stream_progress`'s
/// byte-wise read (the fix for the same `\r` merging bug in that family).
fn stream_progress(
    reader: &mut BufReader<impl std::io::Read>,
    duration_seconds: &mut Option<f64>,
    tail_lines: &mut VecDeque<String>,
    on_progress: &mut dyn FnMut(f32),
) {
    const TAIL_LINES: usize = 40;

    let mut current_line: Vec<u8> = Vec::new();
    let mut byte = [0u8; 1];

    loop {
        match reader.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                let b = byte[0];
                if b == b'\n' || b == b'\r' {
                    if !current_line.is_empty() {
                        let line = String::from_utf8_lossy(&current_line).into_owned();
                        handle_progress_line(&line, duration_seconds, on_progress);
                        tail_lines.push_back(line);
                        if tail_lines.len() > TAIL_LINES {
                            tail_lines.pop_front();
                        }
                        current_line.clear();
                    }
                } else {
                    current_line.push(b);
                }
            }
            Err(_) => break,
        }
    }
    if !current_line.is_empty() {
        let line = String::from_utf8_lossy(&current_line).into_owned();
        handle_progress_line(&line, duration_seconds, on_progress);
        tail_lines.push_back(line);
    }
}

/// Parses one line of FFmpeg stderr: captures the source duration from the
/// first `Duration:` line, then reports `time=.../duration` fractions via
/// `on_progress` once the duration is known.
fn handle_progress_line(line: &str, duration_seconds: &mut Option<f64>, on_progress: &mut dyn FnMut(f32)) {
    if duration_seconds.is_none() {
        if let Some(seconds) = parse_duration_seconds(line) {
            *duration_seconds = Some(seconds);
        }
    }
    if let Some(total) = *duration_seconds {
        if total > 0.0 {
            if let Some(current) = parse_time_progress_seconds(line) {
                let fraction = (current / total).clamp(0.0, 1.0) as f32;
                on_progress(fraction);
            }
        }
    }
}

/// Builds the FFmpeg CLI arguments for converting `source` to `target`
/// (already lowercased), writing to `destination`. Codec choices are sane,
/// broadly-compatible defaults per container — not user-configurable
/// quality settings (that's the later "Compress" tool's job).
fn ffmpeg_args(target: &str, source: &Path, destination: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![OsString::from("-y"), OsString::from("-i"), source.as_os_str().to_os_string()];

    match target {
        // H.264 + AAC is the safe, universally-playable default for
        // MP4/MOV/MKV containers.
        "mp4" | "mov" | "mkv" => {
            args.extend(str_args(&[
                "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac",
            ]));
        }
        // VP9 + Opus, constant-quality mode (-b:v 0 -crf) rather than the
        // slow default bitrate-targeting mode.
        "webm" => {
            args.extend(str_args(&[
                "-c:v", "libvpx-vp9", "-b:v", "0", "-crf", "30", "-pix_fmt", "yuv420p", "-c:a", "libopus",
            ]));
        }
        // MPEG-4 Part 2 + MP3 are AVI's realistic legacy-compatible
        // defaults.
        "avi" => {
            args.extend(str_args(&[
                "-c:v", "mpeg4", "-q:v", "5", "-c:a", "libmp3lame", "-q:a", "4",
            ]));
        }
        // FFmpeg's native WMV2/WMA2 encoders match the container's own
        // era of codecs.
        "wmv" => {
            args.extend(str_args(&["-c:v", "wmv2", "-c:a", "wmav2"]));
        }
        // Two-branch palettegen/paletteuse filter graph: one branch builds
        // a palette from the whole clip, the other is quantized against it
        // — the standard higher-quality alternative to a single naive
        // per-frame quantization pass. GIF has no audio track, so drop it
        // explicitly even though the gif muxer would refuse it anyway.
        "gif" => {
            args.extend(str_args(&[
                "-filter_complex",
                "[0:v] fps=15,split [a][b];[a] palettegen [p];[b][p] paletteuse",
                "-an",
            ]));
        }
        // Audio-only export: drop the video stream entirely.
        "mp3" => {
            args.extend(str_args(&["-vn", "-c:a", "libmp3lame", "-q:a", "2"]));
        }
        _ => {}
    }

    args.push(destination.as_os_str().to_os_string());
    args
}

fn str_args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

/// Parses an FFmpeg `Duration: HH:MM:SS.ms` line (printed once, while
/// probing the input) into total seconds.
fn parse_duration_seconds(line: &str) -> Option<f64> {
    let after = line.split("Duration:").nth(1)?;
    let timestamp = after.trim_start();
    let end = timestamp.find(',').unwrap_or(timestamp.len());
    parse_timestamp(timestamp[..end].trim())
}

/// Parses an FFmpeg progress line's `time=HH:MM:SS.ms` field into seconds.
fn parse_time_progress_seconds(line: &str) -> Option<f64> {
    let after = line.split("time=").nth(1)?;
    let timestamp = after.trim_start();
    let end = timestamp.find(' ').unwrap_or(timestamp.len());
    parse_timestamp(timestamp[..end].trim())
}

/// Parses an `HH:MM:SS.ms` (or `H:MM:SS.ms`) timestamp into total seconds.
fn parse_timestamp(timestamp: &str) -> Option<f64> {
    let mut parts = timestamp.split(':');
    let hours: f64 = parts.next()?.parse().ok()?;
    let minutes: f64 = parts.next()?.parse().ok()?;
    let seconds: f64 = parts.next()?.parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

/// Returns the last `max_len` characters of `s` (on a char boundary), for
/// trimming a captured stderr tail down to something a user can actually
/// read.
fn trim_tail(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        return s.to_string();
    }
    let mut start = s.len() - max_len;
    while !s.is_char_boundary(start) {
        start += 1;
    }
    s[start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;

    /// A fresh, empty temp directory for one test, cleaned up on drop.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!("satsuma-video-test-{label}-{}-{id}", std::process::id()));
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

    /// Locates an actually-runnable FFmpeg binary to exercise conversions
    /// against. Checks the well-known Linux path first, then searches
    /// `PATH` (every candidate, not just the first, so a broken binary
    /// earlier on `PATH` can't mask a working one later). A candidate must
    /// actually execute (`ffmpeg -version` succeeds), not merely exist —
    /// some platforms ship a stub or wrong-architecture `ffmpeg.exe` that
    /// fails to spawn. Returns `None` (rather than panicking) when no
    /// runnable FFmpeg can be found, so this suite skips cleanly on a
    /// machine without it.
    fn find_ffmpeg() -> Option<std::path::PathBuf> {
        let well_known = Path::new("/usr/bin/ffmpeg");
        if well_known.is_file() && ffmpeg_is_runnable(well_known) {
            return Some(well_known.to_path_buf());
        }
        which::which_all("ffmpeg")
            .ok()?
            .find_map(|candidate| ffmpeg_is_runnable(&candidate).then_some(candidate))
    }

    /// `true` if `ffmpeg` actually starts and exits successfully. The
    /// exists-on-disk check alone isn't enough — a broken binary (wrong
    /// architecture, missing DLL, ...) is indistinguishable from a working
    /// one until you try to run it.
    fn ffmpeg_is_runnable(ffmpeg: &Path) -> bool {
        Command::new(ffmpeg)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Only one FFmpeg-invoking test runs at a time — plenty fast for
    /// these sub-second synthetic clips, and avoids CI machines with tight
    /// CPU limits fighting several ffmpeg processes for real-time deadline
    /// filters (the gif palette graph in particular).
    static FFMPEG_SERIAL: Mutex<()> = Mutex::new(());

    /// Acquires [`FFMPEG_SERIAL`], recovering from a poisoned mutex rather
    /// than cascading `PoisonError` failures through the whole suite when
    /// one test panics mid-hold (the first panic still fails its own test
    /// loudly; the rest just serialize as intended instead of every one
    /// failing with the same unrelated cause).
    fn ffmpeg_serial_lock() -> std::sync::MutexGuard<'static, ()> {
        FFMPEG_SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Generates a tiny synthetic video clip (silent unless `with_audio`)
    /// at `dest` using FFmpeg's `lavfi` test sources.
    fn generate_source_clip(ffmpeg: &Path, dest: &Path, with_audio: bool) {
        let mut args: Vec<OsString> = vec![
            OsString::from("-y"),
            OsString::from("-f"),
            OsString::from("lavfi"),
            OsString::from("-i"),
            OsString::from("testsrc=duration=1:size=64x64:rate=5"),
        ];
        if with_audio {
            args.extend(str_args(&["-f", "lavfi", "-i", "sine=frequency=440:duration=1", "-shortest"]));
        }
        args.push(dest.as_os_str().to_os_string());

        let output = Command::new(ffmpeg)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .expect("failed to spawn ffmpeg to generate a test clip");
        assert!(
            output.status.success(),
            "generating test clip failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Generates a tiny synthetic silent GIF at `dest` using FFmpeg.
    fn generate_source_gif(ffmpeg: &Path, dest: &Path) {
        let output = Command::new(ffmpeg)
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=1:size=64x64:rate=5",
                "-filter_complex",
                "[0:v] split [a][b];[a] palettegen [p];[b][p] paletteuse",
            ])
            .arg(dest)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .expect("failed to spawn ffmpeg to generate a test gif");
        assert!(
            output.status.success(),
            "generating test gif failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn supported_targets_matrix_matches_the_format_spec() {
        assert_eq!(
            supported_targets("mp4"),
            &["mov", "mkv", "webm", "avi", "wmv", "gif", "mp3"]
        );
        assert_eq!(supported_targets("gif"), &["mp4", "mov", "mkv", "webm", "avi", "wmv"]);
        assert!(!supported_targets("gif").contains(&"mp3"));
        assert_eq!(supported_targets("unknownext"), Vec::<&str>::new().as_slice());
        // Case-insensitive, matching the rest of the crate's convention.
        assert_eq!(supported_targets("MP4"), supported_targets("mp4"));
    }

    #[test]
    fn convert_rejects_a_target_outside_the_format_matrix() {
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("skipping: no ffmpeg binary found");
            return;
        };
        let _guard = ffmpeg_serial_lock();

        let dir = TempDir::new("unsupported");
        let source = dir.path().join("clip.gif");
        generate_source_gif(&ffmpeg, &source);

        let destination = dir.path().join("clip.mp3");
        let result = convert(&ffmpeg, &source, "mp3", &destination, &mut |_| {});
        assert!(matches!(result, Err(ConvertError::UnsupportedConversion { .. })));
        assert!(!destination.exists());
    }

    #[test]
    fn converts_mp4_to_webm() {
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("skipping: no ffmpeg binary found");
            return;
        };
        let _guard = ffmpeg_serial_lock();

        let dir = TempDir::new("mp4-to-webm");
        let source = dir.path().join("clip.mp4");
        generate_source_clip(&ffmpeg, &source, true);

        let destination = dir.path().join("clip.webm");
        let mut progress_values = Vec::new();
        let result = convert(&ffmpeg, &source, "webm", &destination, &mut |fraction| {
            progress_values.push(fraction);
        });

        assert!(result.is_ok(), "conversion failed: {result:?}");
        assert!(destination.is_file());
        assert!(destination.metadata().unwrap().len() > 0);
        assert!(!progress_values.is_empty(), "on_progress was never called");
        assert!(
            (*progress_values.last().unwrap() - 1.0).abs() < f32::EPSILON,
            "final progress value should be 1.0, got {:?}",
            progress_values.last()
        );
        assert!(probe_has_stream(&ffmpeg, &destination, "video"));
    }

    #[test]
    fn converts_mp4_to_gif() {
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("skipping: no ffmpeg binary found");
            return;
        };
        let _guard = ffmpeg_serial_lock();

        let dir = TempDir::new("mp4-to-gif");
        let source = dir.path().join("clip.mp4");
        generate_source_clip(&ffmpeg, &source, false);

        let destination = dir.path().join("clip.gif");
        let result = convert(&ffmpeg, &source, "gif", &destination, &mut |_| {});

        assert!(result.is_ok(), "conversion failed: {result:?}");
        assert!(destination.is_file());
        assert!(destination.metadata().unwrap().len() > 0);
        // GIF magic bytes: "GIF87a" or "GIF89a".
        let bytes = std::fs::read(&destination).unwrap();
        assert_eq!(&bytes[0..3], b"GIF");
    }

    #[test]
    fn converts_mp4_to_mp3_audio_export() {
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("skipping: no ffmpeg binary found");
            return;
        };
        let _guard = ffmpeg_serial_lock();

        let dir = TempDir::new("mp4-to-mp3");
        let source = dir.path().join("clip.mp4");
        generate_source_clip(&ffmpeg, &source, true);

        let destination = dir.path().join("clip.mp3");
        let result = convert(&ffmpeg, &source, "mp3", &destination, &mut |_| {});

        assert!(result.is_ok(), "conversion failed: {result:?}");
        assert!(destination.is_file());
        assert!(destination.metadata().unwrap().len() > 0);
        assert!(probe_has_stream(&ffmpeg, &destination, "audio"));
        assert!(!probe_has_stream(&ffmpeg, &destination, "video"));
    }

    #[test]
    fn converts_silent_gif_to_mp4() {
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("skipping: no ffmpeg binary found");
            return;
        };
        let _guard = ffmpeg_serial_lock();

        let dir = TempDir::new("gif-to-mp4");
        let source = dir.path().join("clip.gif");
        generate_source_gif(&ffmpeg, &source);

        let destination = dir.path().join("clip.mp4");
        let result = convert(&ffmpeg, &source, "mp4", &destination, &mut |_| {});

        assert!(result.is_ok(), "conversion failed: {result:?}");
        assert!(destination.is_file());
        assert!(destination.metadata().unwrap().len() > 0);
        assert!(probe_has_stream(&ffmpeg, &destination, "video"));
    }

    #[test]
    fn progress_callback_reports_increasing_ish_values() {
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("skipping: no ffmpeg binary found");
            return;
        };
        let _guard = ffmpeg_serial_lock();

        let dir = TempDir::new("progress");
        let source = dir.path().join("clip.mp4");
        generate_source_clip(&ffmpeg, &source, true);

        let destination = dir.path().join("clip.mkv");
        let mut progress_values: Vec<f32> = Vec::new();
        let result = convert(&ffmpeg, &source, "mkv", &destination, &mut |fraction| {
            progress_values.push(fraction);
        });

        assert!(result.is_ok(), "conversion failed: {result:?}");
        assert!(!progress_values.is_empty());
        // Every value should be a valid fraction, and the sequence should
        // never decrease (FFmpeg's own timestamps are monotonic).
        for pair in progress_values.windows(2) {
            assert!(pair[1] + f32::EPSILON >= pair[0], "progress went backwards: {progress_values:?}");
        }
        for value in &progress_values {
            assert!((0.0..=1.0).contains(value));
        }
    }

    /// A crafted FFmpeg-stderr-shaped payload: a `Duration:` line, then
    /// several `\r`-terminated intermediate stats (FFmpeg's real shape for
    /// running progress — only the final summary ends in `\n`) followed by a
    /// final plain `\n`-terminated line. Exactly the input `BufRead::lines()`
    /// merged into a single line.
    const SAMPLE_STDERR: &str = "\
Input #0, lavfi, from 'testsrc=duration=3:size=64x64:rate=5':
  Duration: 00:00:03.00, start: 0.000000, bitrate: N/A
frame=    5 fps=0.0 q=0.0 size=       0kB time=00:00:01.00 bitrate=   0.0kbits/s speed=1.0x\r\
frame=   10 fps=0.0 q=0.0 size=       0kB time=00:00:02.00 bitrate=   0.0kbits/s speed=1.0x\r\
frame=   15 fps=0.0 q=0.0 size=       0kB time=00:00:03.00 bitrate=   0.0kbits/s speed=1.0x\r\
frame=   15 fps=0.0 q=0.0 size=       0kB time=00:00:03.00 bitrate=   0.0kbits/s speed=1.0x";

    #[test]
    fn stream_progress_reports_each_cr_terminated_update_separately() {
        let mut reader = BufReader::new(std::io::Cursor::new(SAMPLE_STDERR.as_bytes()));
        let mut duration_seconds = None;
        let mut tail_lines = VecDeque::new();
        let mut fractions = Vec::new();
        stream_progress(&mut reader, &mut duration_seconds, &mut tail_lines, &mut |fraction| fractions.push(fraction));

        // Regression for the `BufRead::lines()` bug: it split on `\n` only,
        // so the `\r`-terminated stats all merged into one line and
        // `parse_time_progress_seconds` only ever saw the first `time=` — one
        // early blip, then nothing until the final 1.0. Splitting on `\r`
        // too must surface every intermediate update individually.
        assert_eq!(duration_seconds, Some(3.0));
        assert_eq!(fractions.len(), 4, "expected every `\r`-terminated stat to be its own fraction: {fractions:?}");
        for (actual, expected) in fractions.iter().zip([1.0 / 3.0, 2.0 / 3.0, 1.0, 1.0]) {
            assert!(
                (actual - expected as f32).abs() < 1e-4,
                "fraction {actual} != expected {expected}, got {fractions:?}"
            );
        }
        // Each update is also its own tail line for error reporting (the `\r`
        // characters are stripped), not one giant merged blob.
        assert_eq!(tail_lines.len(), 6);
        assert!(tail_lines.iter().all(|line| !line.contains('\r')));
        assert!(tail_lines.iter().any(|line| line.contains("Duration:")));
    }

    /// Uses `ffmpeg` itself (via `-i` on a null muxer, parsing stderr) to
    /// check whether `path` has at least one stream of the given
    /// `stream_type` ("video" or "audio") — a lightweight stand-in for
    /// `ffprobe`, which may not be installed alongside a minimal `ffmpeg`.
    fn probe_has_stream(ffmpeg: &Path, path: &Path, stream_type: &str) -> bool {
        let output = Command::new(ffmpeg)
            .args(["-hide_banner", "-i"])
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .expect("failed to spawn ffmpeg to probe output file");
        let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
        let needle = format!("{stream_type}: ");
        stderr
            .lines()
            .any(|line| line.contains("stream #0") && line.contains(&needle))
    }
}
