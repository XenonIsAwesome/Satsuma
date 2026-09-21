//! Audio <-> audio conversion (MP3/M4A/WAV/FLAC/OGG/Opus/AIFF/WMA), via the
//! same bundled FFmpeg sidecar binary as [`super::video`]. The binary's
//! path is passed in by the Tauri layer rather than looked up here, so this
//! module stays Tauri-free and testable by pointing it at any FFmpeg
//! binary (e.g. the system one, in tests).

use super::error::ConvertError;
use std::io::Read;
use std::path::Path;
use std::process::Child;

/// Every audio extension this module round-trips between, in the canonical
/// spelling used for target extensions (`"aiff"`, never `"aif"`).
const FORMATS: &[&str] = &["mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff", "wma"];

/// Extensions this module can produce from `source_ext`, excluding
/// `source_ext` itself.
pub fn supported_targets(source_ext: &str) -> &'static [&'static str] {
    match canonicalize(source_ext) {
        "mp3" => &["m4a", "wav", "flac", "ogg", "opus", "aiff", "wma"],
        "m4a" => &["mp3", "wav", "flac", "ogg", "opus", "aiff", "wma"],
        "wav" => &["mp3", "m4a", "flac", "ogg", "opus", "aiff", "wma"],
        "flac" => &["mp3", "m4a", "wav", "ogg", "opus", "aiff", "wma"],
        "ogg" => &["mp3", "m4a", "wav", "flac", "opus", "aiff", "wma"],
        "opus" => &["mp3", "m4a", "wav", "flac", "ogg", "aiff", "wma"],
        "aiff" => &["mp3", "m4a", "wav", "flac", "ogg", "opus", "wma"],
        "wma" => &["mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff"],
        _ => &[],
    }
}

/// Converts `source` to `target_extension` using the FFmpeg binary at
/// `ffmpeg_bin`, writing to `destination`. `on_progress` is called with a
/// `0.0..=1.0` fraction as FFmpeg reports encode progress against the
/// source's known duration (parsed from FFmpeg's own stderr), and is always
/// called at least once with something close to `1.0` on success.
pub fn convert(
    ffmpeg_bin: &Path,
    source: &Path,
    target_extension: &str,
    destination: &Path,
    on_progress: &mut dyn FnMut(f32),
) -> Result<(), ConvertError> {
    let target = target_extension.to_ascii_lowercase();
    let source_ext = source
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();

    if !supported_targets(&source_ext).contains(&target.as_str()) {
        return Err(ConvertError::UnsupportedConversion {
            from: source_ext,
            to: target,
        });
    }
    if !source.is_file() {
        return Err(ConvertError::InvalidSource(format!(
            "not a file: {}",
            source.display()
        )));
    }

    let mut command = super::build_ffmpeg_command(ffmpeg_bin);
    command.arg("-y").arg("-i").arg(source).arg("-vn");
    for arg in codec_args(&target) {
        command.arg(arg);
    }
    command.arg(destination);

    let mut child = command
        .spawn()
        .map_err(|error| ConvertError::EngineUnavailable(format!("failed to launch FFmpeg: {error}")))?;

    let (captured_stderr, reported_progress) = stream_progress(&mut child, on_progress);

    let status = child
        .wait()
        .map_err(|error| ConvertError::Other(format!("FFmpeg process error: {error}")))?;

    if !status.success() || !destination.exists() {
        return Err(ConvertError::Other(trimmed_tail(&captured_stderr)));
    }

    // Always end at (close to) 1.0, per the progress contract, whether or
    // not any intermediate fraction could be computed from FFmpeg's own
    // stderr (e.g. a source too short to cross a stats interval).
    let _ = reported_progress;
    on_progress(1.0);

    Ok(())
}

/// The `-c:a`/muxer-hint arguments FFmpeg needs for `target`, assumed
/// already validated against [`supported_targets`].
fn codec_args(target: &str) -> Vec<&'static str> {
    match target {
        "mp3" => vec!["-c:a", "libmp3lame"],
        // FFmpeg 6.1 (this module's test environment) already infers the
        // MP4/M4A container correctly from a bare `.m4a` output path when
        // muxing AAC audio-only content -- verified byte-for-byte identical
        // output with and without this flag. `-f ipod` (FFmpeg's own muxer
        // alias for that container) is kept explicit anyway as a defensive
        // hint for older/other FFmpeg builds that might not infer it, since
        // it's a no-op when the inference already works.
        "m4a" => vec!["-c:a", "aac", "-f", "ipod"],
        "wav" => vec!["-c:a", "pcm_s16le"],
        "flac" => vec!["-c:a", "flac"],
        "ogg" => vec!["-c:a", "libvorbis"],
        // `.opus` is conventionally an Ogg Opus container; FFmpeg has a
        // dedicated `opus` muxer for exactly this (selected automatically
        // from the `.opus` extension, verified empirically in this module's
        // tests).
        "opus" => vec!["-c:a", "libopus"],
        // AIFF's traditional format is big-endian PCM.
        "aiff" => vec!["-c:a", "pcm_s16be"],
        "wma" => vec!["-c:a", "wmav2"],
        _ => Vec::new(),
    }
}

/// Maps an `aif`/`aiff` spelling ambiguity to the canonical `"aiff"` used by
/// [`FORMATS`] and the match arms above; every other extension passes
/// through lowercased and unchanged.
fn canonicalize(source_ext: &str) -> &'static str {
    let lower = source_ext.to_ascii_lowercase();
    let lower = if lower == "aif" { "aiff".to_string() } else { lower };
    match FORMATS.iter().find(|&&format| format == lower) {
        Some(&format) => format,
        None => "",
    }
}

/// Reads `child`'s stderr line-by-line (splitting on FFmpeg's own mix of
/// `\n`-terminated log lines and `\r`-overwritten progress lines) while the
/// process is still running, parsing `Duration:`/`time=` timestamps to drive
/// `on_progress`. Returns the full captured stderr text (for error
/// reporting) and whether at least one intermediate fraction was reported.
fn stream_progress(child: &mut Child, on_progress: &mut dyn FnMut(f32)) -> (String, bool) {
    let mut captured = String::new();
    let mut reported = false;

    let Some(mut stderr) = child.stderr.take() else {
        return (captured, reported);
    };

    let mut duration_seconds: Option<f64> = None;
    let mut current_line: Vec<u8> = Vec::new();
    let mut byte = [0u8; 1];

    loop {
        match stderr.read(&mut byte) {
            Ok(0) => break,
            Ok(_) => {
                let b = byte[0];
                if b == b'\n' || b == b'\r' {
                    if !current_line.is_empty() {
                        let line = String::from_utf8_lossy(&current_line).into_owned();
                        captured.push_str(&line);
                        captured.push('\n');
                        handle_line(&line, &mut duration_seconds, on_progress, &mut reported);
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
        captured.push_str(&line);
        handle_line(&line, &mut duration_seconds, on_progress, &mut reported);
    }

    (captured, reported)
}

/// Parses one line of FFmpeg stderr, updating `duration_seconds` from a
/// `Duration:` line or reporting a fraction from a `time=` line.
fn handle_line(line: &str, duration_seconds: &mut Option<f64>, on_progress: &mut dyn FnMut(f32), reported: &mut bool) {
    if duration_seconds.is_none() {
        if let Some(seconds) = parse_timestamp_after(line, "Duration: ") {
            *duration_seconds = Some(seconds);
        }
    }
    if let Some(total) = *duration_seconds {
        if total > 0.0 {
            if let Some(current) = parse_timestamp_after(line, "time=") {
                let fraction = (current / total).clamp(0.0, 1.0) as f32;
                on_progress(fraction);
                *reported = true;
            }
        }
    }
}

/// Finds `marker` in `line` and parses the `HH:MM:SS.ms` timestamp token
/// that immediately follows it (up to the next comma or whitespace) into
/// seconds. Returns `None` if `marker` isn't present or the token isn't a
/// well-formed timestamp (e.g. FFmpeg's own `time=N/A`).
fn parse_timestamp_after(line: &str, marker: &str) -> Option<f64> {
    let start = line.find(marker)? + marker.len();
    let rest = line[start..].trim_start();
    let end = rest.find(|c: char| c == ',' || c.is_whitespace()).unwrap_or(rest.len());
    parse_hms(&rest[..end])
}

/// Parses an `HH:MM:SS.ms` token into total seconds.
fn parse_hms(token: &str) -> Option<f64> {
    let mut parts = token.split(':');
    let hours: f64 = parts.next()?.parse().ok()?;
    let minutes: f64 = parts.next()?.parse().ok()?;
    let seconds: f64 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}

/// Returns a trimmed, readable slice of FFmpeg's captured stderr suitable
/// for surfacing in a [`ConvertError::Other`] — the last ~500 characters
/// rather than the full raw log dump.
fn trimmed_tail(output: &str) -> String {
    const MAX_LEN: usize = 500;
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return "FFmpeg exited with an error and produced no output".to_string();
    }
    if trimmed.len() <= MAX_LEN {
        return trimmed.to_string();
    }
    let mut start = trimmed.len() - MAX_LEN;
    while !trimmed.is_char_boundary(start) {
        start += 1;
    }
    format!("...{}", &trimmed[start..])
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
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let id = COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!("satsuma-audio-test-{}-{id}", std::process::id()));
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

    /// Locates an actually-runnable FFmpeg binary for tests: `/usr/bin/ffmpeg`
    /// first (guaranteed present in CI/sandbox per the project's test setup),
    /// then a `PATH` search via the `which` crate (every candidate, not just
    /// the first — a broken binary earlier on `PATH` must not mask a working
    /// one later). A candidate must actually execute (`ffmpeg -version`
    /// succeeds), not merely exist: some platforms ship a stub or
    /// wrong-architecture `ffmpeg.exe` on `PATH` that fails to spawn, which
    /// would otherwise panic every conversion test that "found" it. Returns
    /// `None` (with a clear skip message) if genuinely absent, so the suite
    /// degrades gracefully rather than failing everywhere else.
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

    /// Generates a tiny synthetic sine-wave WAV file at `path` using
    /// FFmpeg's `lavfi` test source, `duration_secs` long.
    fn generate_sine_wav(ffmpeg: &Path, path: &Path, duration_secs: u32) {
        let status = Command::new(ffmpeg)
            .args(["-f", "lavfi", "-i"])
            .arg(format!("sine=frequency=440:duration={duration_secs}"))
            .arg("-y")
            .arg(path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("failed to run ffmpeg to generate test fixture");
        assert!(status.success(), "ffmpeg failed to generate sine wave fixture");
        assert!(path.exists(), "sine wave fixture was not written");
    }

    /// Runs `ffmpeg -i <path>` (a decode-only probe, no output file) and
    /// returns its captured stderr, which contains FFmpeg's own format/
    /// codec/duration report — used to assert the produced file really is
    /// a valid file of the expected format, not just bytes with the right
    /// extension.
    fn probe(ffmpeg: &Path, path: &Path) -> String {
        let output = Command::new(ffmpeg)
            .arg("-i")
            .arg(path)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .expect("failed to probe output file");
        String::from_utf8_lossy(&output.stderr).into_owned()
    }

    // FFmpeg test invocations spawn real child processes; running many
    // conversions concurrently in a sandbox with limited CPU can make
    // individual encodes flaky/slow. Serialize the FFmpeg-touching tests.
    static FFMPEG_TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Acquires [`FFMPEG_TEST_LOCK`], recovering from a poisoned mutex
    /// rather than cascading `PoisonError` failures through the whole suite
    /// when one test panics mid-hold (the first panic still fails its own
    /// test loudly; the rest just serialize as intended instead of every
    /// one failing with the same unrelated cause).
    fn ffmpeg_test_lock() -> std::sync::MutexGuard<'static, ()> {
        FFMPEG_TEST_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn supported_targets_excludes_the_source_and_lists_the_other_seven() {
        for format in FORMATS {
            let targets = supported_targets(format);
            assert_eq!(targets.len(), 7, "wrong target count for {format}");
            assert!(!targets.contains(format), "{format} should not target itself");
            for other in FORMATS {
                if other != format {
                    assert!(targets.contains(other), "{format} -> {other} missing");
                }
            }
        }
    }

    #[test]
    fn supported_targets_treats_aif_as_aiff() {
        assert_eq!(supported_targets("aif"), supported_targets("aiff"));
    }

    #[test]
    fn supported_targets_is_case_insensitive() {
        assert_eq!(supported_targets("MP3"), supported_targets("mp3"));
    }

    #[test]
    fn supported_targets_is_empty_for_unknown_extension() {
        assert_eq!(supported_targets("xyz"), &[] as &[&str]);
    }

    #[test]
    fn convert_rejects_unsupported_target() {
        let _guard = ffmpeg_test_lock();
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("SKIP: ffmpeg not found on this system");
            return;
        };
        let dir = TempDir::new();
        let source = dir.path().join("tone.wav");
        generate_sine_wav(&ffmpeg, &source, 1);

        let destination = dir.path().join("tone.zzz");
        let result = convert(&ffmpeg, &source, "zzz", &destination, &mut |_| {});
        assert!(matches!(result, Err(ConvertError::UnsupportedConversion { .. })));
        assert!(!destination.exists());
    }

    #[test]
    fn convert_rejects_missing_source() {
        let _guard = ffmpeg_test_lock();
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("SKIP: ffmpeg not found on this system");
            return;
        };
        let dir = TempDir::new();
        let source = dir.path().join("does-not-exist.wav");
        let destination = dir.path().join("out.mp3");

        let result = convert(&ffmpeg, &source, "mp3", &destination, &mut |_| {});
        assert!(matches!(
            result,
            Err(ConvertError::InvalidSource(_)) | Err(ConvertError::UnsupportedConversion { .. })
        ));
    }

    /// Runs one real conversion, asserting the destination exists,
    /// non-empty, and that FFmpeg's own probe of it reports the expected
    /// codec. Returns the collected progress fractions.
    fn run_conversion(
        ffmpeg: &Path,
        source_ext: &str,
        target_ext: &str,
        duration_secs: u32,
        expect_codec: &str,
    ) -> Vec<f32> {
        let dir = TempDir::new();
        let source = dir.path().join(format!("tone.{source_ext}"));
        if source_ext == "wav" {
            generate_sine_wav(ffmpeg, &source, duration_secs);
        } else {
            // Build the source itself via a WAV -> source_ext conversion,
            // so every pairwise test still originates from the same lavfi
            // sine wave.
            let wav_source = dir.path().join("seed.wav");
            generate_sine_wav(ffmpeg, &wav_source, duration_secs);
            let mut progress = vec![];
            convert(ffmpeg, &wav_source, source_ext, &source, &mut |f| progress.push(f))
                .unwrap_or_else(|error| panic!("failed to build {source_ext} fixture: {error}"));
        }

        let destination = dir.path().join(format!("out.{target_ext}"));
        let mut progress = Vec::new();
        let result = convert(ffmpeg, &source, target_ext, &destination, &mut |f| progress.push(f));
        assert!(result.is_ok(), "{source_ext} -> {target_ext} failed: {:?}", result.err());

        assert!(destination.exists(), "{source_ext} -> {target_ext}: destination missing");
        let metadata = std::fs::metadata(&destination).unwrap();
        assert!(metadata.len() > 0, "{source_ext} -> {target_ext}: destination is empty");

        let probe_output = probe(ffmpeg, &destination);
        assert!(
            probe_output.to_lowercase().contains(&expect_codec.to_lowercase()),
            "{source_ext} -> {target_ext}: probe output did not mention codec {expect_codec}:\n{probe_output}"
        );

        progress
    }

    #[test]
    fn wav_to_mp3_lossless_to_lossy() {
        let _guard = ffmpeg_test_lock();
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("SKIP: ffmpeg not found on this system");
            return;
        };
        run_conversion(&ffmpeg, "wav", "mp3", 2, "mp3");
    }

    #[test]
    fn mp3_to_flac_lossy_to_lossless() {
        let _guard = ffmpeg_test_lock();
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("SKIP: ffmpeg not found on this system");
            return;
        };
        run_conversion(&ffmpeg, "mp3", "flac", 2, "flac");
    }

    #[test]
    fn flac_to_ogg_lossless_to_lossy() {
        let _guard = ffmpeg_test_lock();
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("SKIP: ffmpeg not found on this system");
            return;
        };
        run_conversion(&ffmpeg, "flac", "ogg", 2, "vorbis");
    }

    #[test]
    fn ogg_to_opus_same_family() {
        let _guard = ffmpeg_test_lock();
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("SKIP: ffmpeg not found on this system");
            return;
        };
        run_conversion(&ffmpeg, "ogg", "opus", 2, "opus");
    }

    #[test]
    fn wav_to_aiff_lossless_to_lossless() {
        let _guard = ffmpeg_test_lock();
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("SKIP: ffmpeg not found on this system");
            return;
        };
        run_conversion(&ffmpeg, "wav", "aiff", 2, "pcm_s16be");
    }

    #[test]
    fn mp3_to_wma_lossy_to_lossy() {
        let _guard = ffmpeg_test_lock();
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("SKIP: ffmpeg not found on this system");
            return;
        };
        run_conversion(&ffmpeg, "mp3", "wma", 2, "wmav2");
    }

    #[test]
    fn wav_to_m4a_aac_container() {
        let _guard = ffmpeg_test_lock();
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("SKIP: ffmpeg not found on this system");
            return;
        };
        run_conversion(&ffmpeg, "wav", "m4a", 2, "aac");
    }

    #[test]
    fn m4a_round_trips_back_to_wav() {
        let _guard = ffmpeg_test_lock();
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("SKIP: ffmpeg not found on this system");
            return;
        };
        run_conversion(&ffmpeg, "m4a", "wav", 2, "pcm_s16le");
    }

    #[test]
    fn progress_callback_is_called_and_ends_near_one() {
        let _guard = ffmpeg_test_lock();
        let Some(ffmpeg) = find_ffmpeg() else {
            eprintln!("SKIP: ffmpeg not found on this system");
            return;
        };
        let dir = TempDir::new();
        let source = dir.path().join("tone.wav");
        // A few seconds so FFmpeg has more than one chance to cross a
        // stats-reporting interval and emit an intermediate `time=` line.
        generate_sine_wav(&ffmpeg, &source, 3);

        let destination = dir.path().join("tone.mp3");
        let mut progress = Vec::new();
        let result = convert(&ffmpeg, &source, "mp3", &destination, &mut |f| progress.push(f));
        assert!(result.is_ok(), "conversion failed: {:?}", result.err());

        assert!(!progress.is_empty(), "on_progress was never called");
        let last = *progress.last().unwrap();
        assert!(last >= 0.99, "final progress {last} was not close to 1.0");

        // Non-decreasing (allow ties, since multiple stats lines can land
        // on the same rounded second for a short clip).
        for pair in progress.windows(2) {
            assert!(pair[0] <= pair[1], "progress went backwards: {:?}", progress);
        }
        for &value in &progress {
            assert!((0.0..=1.0).contains(&value), "progress {value} out of range");
        }
    }
}
