// SPDX-License-Identifier: GPL-3.0

//! Running external package tools.
//!
//! Every backend in this application ultimately shells out to something —
//! `dpkg-deb`, `apt-get`, `flatpak`, `rpm`. They all want the same three things,
//! which is why they go through here rather than using [`std::process`] directly:
//!
//! * **A forced C locale.** apt and dpkg translate their output. A parser
//!   written against English output silently breaks in every other locale, and
//!   the breakage only shows up for users who don't share the developer's
//!   language. [`C_LOCALE`] is applied to every command without exception.
//! * **A timeout.** A hung child would otherwise wedge a worker thread forever
//!   with no way for the user to recover short of killing the window.
//! * **Streaming output.** Install operations can run for minutes and the user
//!   deserves to see progress, so output is delivered line by line as it
//!   arrives instead of only at exit.

use std::{
    ffi::OsStr,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use crate::constants::C_LOCALE;
use crate::debug_log;
use crate::debug::EXEC;

/// How often a running child is checked for exit once its output has closed.
///
/// Only reached after both pipes hit EOF, which for a well-behaved program is
/// immediately before it exits, so this rarely costs more than a single sleep.
const REAP_POLL_INTERVAL: Duration = Duration::from_millis(20);

/// Which pipe a line of output came from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stream {
    Stdout,
    Stderr,
}

/// The result of a finished command.
#[derive(Clone, Debug)]
pub struct Output {
    /// Exit code, or `None` if the process was killed by a signal.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    /// Whether the command exited cleanly.
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    /// The most useful line to show a user when the command failed.
    ///
    /// Prefers stderr, since that is where these tools put their diagnostics,
    /// but falls back to stdout because `apt-get` reports unmet dependencies —
    /// the single most likely failure here — on stdout.
    pub fn failure_message(&self) -> String {
        let pick = |text: &str| -> Option<String> {
            let collected: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
            if collected.is_empty() {
                None
            } else {
                Some(collected.join("\n"))
            }
        };
        pick(&self.stderr)
            .or_else(|| pick(&self.stdout))
            .unwrap_or_default()
    }
}

/// Why a command could not be run to completion.
#[derive(Debug)]
pub enum ExecError {
    /// The program is not installed, or could not be started.
    Spawn {
        program: String,
        source: std::io::Error,
    },
    /// The program ran longer than its allotted time and was killed.
    Timeout { program: String, after: Duration },
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn { program, source } => write!(f, "failed to run `{program}`: {source}"),
            Self::Timeout { program, after } => {
                write!(f, "`{program}` timed out after {:?}", after)
            }
        }
    }
}

impl std::error::Error for ExecError {}

/// Whether `program` can be found on `PATH`.
///
/// Used to decide which package formats the running system can handle at all,
/// so the UI can say "install rpm to open this file" instead of failing at the
/// point the user presses Install.
pub fn have(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        let candidate = dir.join(program);
        // Directories can be executable too, hence the explicit file check.
        candidate.is_file() && is_executable(&candidate)
    })
}

#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_path: &std::path::Path) -> bool {
    true
}

/// Run `program` to completion, collecting its output.
pub fn run<S: AsRef<OsStr>>(
    program: &str,
    args: &[S],
    timeout: Duration,
) -> Result<Output, ExecError> {
    run_streaming(program, args, timeout, |_, _| {})
}

/// Run `program` with its working directory set to `dir`.
///
/// Exists for one job: an AppImage's `--appimage-extract` writes into a
/// `squashfs-root` directory beside wherever it happens to be run from, with no
/// option to say where. Pointing the child at a temporary directory is the only
/// way to stop it depositing that in whatever directory the application was
/// launched from.
pub fn run_in_dir<S: AsRef<OsStr>>(
    program: &str,
    args: &[S],
    dir: &std::path::Path,
    timeout: Duration,
) -> Result<Output, ExecError> {
    run_inner(program, args, Some(dir), timeout, |_, _| {})
}

/// Run `program` to completion, invoking `on_line` for each line of output as
/// it arrives, and also collecting that output into the returned [`Output`].
///
/// The callback runs on the calling thread between reads, so it must not block
/// for long — anything expensive belongs on the receiving end of a channel.
pub fn run_streaming<S: AsRef<OsStr>, F: FnMut(Stream, &str)>(
    program: &str,
    args: &[S],
    timeout: Duration,
    on_line: F,
) -> Result<Output, ExecError> {
    run_inner(program, args, None, timeout, on_line)
}

fn run_inner<S: AsRef<OsStr>, F: FnMut(Stream, &str)>(
    program: &str,
    args: &[S],
    dir: Option<&std::path::Path>,
    timeout: Duration,
    mut on_line: F,
) -> Result<Output, ExecError> {
    let started = Instant::now();
    debug_log!(
        EXEC,
        "run {program} {:?}",
        args.iter()
            .map(|a| a.as_ref().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
    );

    let mut command = Command::new(program);
    command
        .args(args)
        .envs(C_LOCALE)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = dir {
        command.current_dir(dir);
    }

    let mut child = command.spawn().map_err(|source| {
        debug_log!(EXEC, "spawn of {program} failed: {source}");
        ExecError::Spawn {
            program: program.to_string(),
            source,
        }
    })?;

    // Both pipes are drained by dedicated threads. Polling them from here
    // instead would let a full pipe buffer block the child while we wait on the
    // other stream — a deadlock that only shows up on verbose output.
    let (tx, rx) = mpsc::channel::<(Stream, String)>();
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(spawn_reader(Stream::Stdout, stdout, tx.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(spawn_reader(Stream::Stderr, stderr, tx.clone()));
    }
    // The loop below ends on channel disconnect, which cannot happen while this
    // original sender is alive.
    drop(tx);

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut timed_out = false;

    loop {
        let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
            timed_out = true;
            break;
        };
        match rx.recv_timeout(remaining) {
            Ok((stream, line)) => {
                on_line(stream, &line);
                let buffer = match stream {
                    Stream::Stdout => &mut stdout,
                    Stream::Stderr => &mut stderr,
                };
                buffer.push_str(&line);
                buffer.push('\n');
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                timed_out = true;
                break;
            }
            // Both readers have hit EOF, so the child is done writing.
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    // Wait for the exit status, still bounded by the same deadline.
    let mut code = None;
    if !timed_out {
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    code = status.code();
                    break;
                }
                Ok(None) => {
                    if started.elapsed() >= timeout {
                        timed_out = true;
                        break;
                    }
                    thread::sleep(REAP_POLL_INTERVAL);
                }
                Err(source) => {
                    debug_log!(EXEC, "wait on {program} failed: {source}");
                    return Err(ExecError::Spawn {
                        program: program.to_string(),
                        source,
                    });
                }
            }
        }
    }

    if timed_out {
        debug_log!(EXEC, "{program} timed out after {:?}, killing", timeout);
        let _ = child.kill();
        let _ = child.wait();
        // Reader threads end once the pipes close with the child.
        for reader in readers {
            let _ = reader.join();
        }
        return Err(ExecError::Timeout {
            program: program.to_string(),
            after: timeout,
        });
    }

    for reader in readers {
        let _ = reader.join();
    }

    debug_log!(
        EXEC,
        "{program} exited {:?} in {:.3}s ({} bytes out, {} bytes err)",
        code,
        started.elapsed().as_secs_f64(),
        stdout.len(),
        stderr.len()
    );

    Ok(Output {
        code,
        stdout,
        stderr,
    })
}

/// Read `source` line by line, forwarding each line until EOF.
fn spawn_reader<R: std::io::Read + Send + 'static>(
    stream: Stream,
    source: R,
    tx: mpsc::Sender<(Stream, String)>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for line in BufReader::new(source).lines() {
            // Invalid UTF-8 in a path or a maintainer name should not lose the
            // rest of the output, but there is nothing sensible to do with the
            // failing line itself.
            let Ok(line) = line else { continue };
            if tx.send((stream, line)).is_err() {
                // The consumer gave up (timeout); no point reading further.
                break;
            }
        }
    })
}

/// Run `producer` and feed its standard output into `consumer`, returning what
/// `consumer` wrote as raw bytes.
///
/// Exists for one job: pulling a single file out of a package payload, which
/// on Debian means `dpkg-deb --fsys-tarfile pkg.deb | tar -xO ./path`. Output
/// is bytes rather than text because the extracted file is usually an icon.
pub fn run_piped<S: AsRef<OsStr>, T: AsRef<OsStr>>(
    producer: &str,
    producer_args: &[S],
    consumer: &str,
    consumer_args: &[T],
    timeout: Duration,
) -> Result<Vec<u8>, ExecError> {
    let started = Instant::now();
    debug_log!(EXEC, "pipe {producer} | {consumer}");

    let mut left = Command::new(producer)
        .args(producer_args)
        .envs(C_LOCALE)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| ExecError::Spawn {
            program: producer.to_string(),
            source,
        })?;

    // Safe to unwrap: stdout was configured as a pipe just above and nothing
    // has taken it yet.
    let left_stdout = left.stdout.take().expect("producer stdout was piped");

    let right = Command::new(consumer)
        .args(consumer_args)
        .envs(C_LOCALE)
        .stdin(Stdio::from(left_stdout))
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();

    let mut right = match right {
        Ok(child) => child,
        Err(source) => {
            let _ = left.kill();
            let _ = left.wait();
            return Err(ExecError::Spawn {
                program: consumer.to_string(),
                source,
            });
        }
    };

    // Drain the consumer on a worker so a large payload cannot fill the pipe
    // while this thread is polling for exit.
    let mut consumer_stdout = right.stdout.take().expect("consumer stdout was piped");
    let collector = thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = std::io::Read::read_to_end(&mut consumer_stdout, &mut buffer);
        buffer
    });

    loop {
        match right.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if started.elapsed() >= timeout {
                    debug_log!(EXEC, "pipe {producer} | {consumer} timed out");
                    let _ = right.kill();
                    let _ = right.wait();
                    let _ = left.kill();
                    let _ = left.wait();
                    let _ = collector.join();
                    return Err(ExecError::Timeout {
                        program: consumer.to_string(),
                        after: timeout,
                    });
                }
                thread::sleep(REAP_POLL_INTERVAL);
            }
            Err(source) => {
                let _ = left.kill();
                let _ = left.wait();
                let _ = collector.join();
                return Err(ExecError::Spawn {
                    program: consumer.to_string(),
                    source,
                });
            }
        }
    }

    // `tar -xO` exits as soon as it has the member it wants, leaving the
    // producer writing into a closed pipe. That is expected, not an error, so
    // the producer is simply stopped and reaped.
    let _ = left.kill();
    let _ = left.wait();

    let bytes = collector.join().unwrap_or_default();
    debug_log!(
        EXEC,
        "pipe {producer} | {consumer} produced {} bytes in {:.3}s",
        bytes.len(),
        started.elapsed().as_secs_f64()
    );
    Ok(bytes)
}
