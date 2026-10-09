//! HTTP: small JSON requests (GitHub, AUR) and resumable file downloads.

use crate::util::{Log, Msg, human_size};
use anyhow::{Context, Result, bail};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::{RecvTimeoutError, sync_channel};
use std::time::Duration;

/// Give up on a download attempt if no data arrives for this long.
const STALL_TIMEOUT: Duration = Duration::from_secs(30);
const ATTEMPTS: usize = 5;

fn agent(timeout: Option<Duration>) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(timeout)
        .timeout_connect(Some(Duration::from_secs(15)))
        .user_agent(concat!("stash/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// GET a URL and parse the JSON response.
pub fn get_json<T: serde::de::DeserializeOwned>(url: &str, timeout: Duration) -> Result<T> {
    let mut request = agent(Some(timeout)).get(url).header("Accept", "application/vnd.github+json");
    // Optional: a GitHub token raises the API limit from 60 to 5000 requests/hour.
    if url.starts_with("https://api.github.com/")
        && let Ok(token) = std::env::var("GITHUB_TOKEN")
    {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    let mut response = request.call().with_context(|| format!("request to {url} failed"))?;
    Ok(response.body_mut().read_json()?)
}

/// What the download thread tells the main thread.
enum Event {
    /// Response received. `total` is the full file size if known;
    /// `resumed` is false if the server ignored our request to continue.
    Started { total: Option<u64>, resumed: bool },
    Data(Vec<u8>),
    Failed(String),
}

/// Download `url` to `dest`, showing progress. If the connection stalls or
/// drops, it retries and continues where it stopped instead of starting over.
pub fn download(url: &str, dest: &Path, log: Log) -> Result<()> {
    let name = url.rsplit('/').next().unwrap_or(url);
    log(Msg::Step(format!("Downloading {name}…")));

    let mut last_error = String::new();
    for attempt in 1..=ATTEMPTS {
        match download_attempt(url, dest, log) {
            Ok(()) => {
                let size = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
                log(Msg::Done(format!("Downloaded {}", human_size(size))));
                return Ok(());
            }
            Err(error) => {
                last_error = format!("{error:#}");
                if attempt < ATTEMPTS {
                    log(Msg::Warn(format!("{last_error}; retrying ({attempt}/{ATTEMPTS})…")));
                    std::thread::sleep(Duration::from_secs(2));
                }
            }
        }
    }
    bail!("Download failed: {last_error}")
}

fn download_attempt(url: &str, dest: &Path, log: Log) -> Result<()> {
    let already = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);

    // The network read happens on its own thread, which sends us the data.
    // A read that hangs can't be interrupted, but we can stop waiting for it:
    // if nothing arrives for STALL_TIMEOUT we give up on this attempt (the
    // thread exits by itself once it notices nobody is listening).
    let (sender, receiver) = sync_channel::<Event>(32);
    let url = url.to_string();
    std::thread::spawn(move || {
        let mut request = agent(None).get(&url);
        if already > 0 {
            request = request.header("Range", format!("bytes={already}-"));
        }
        let mut response = match request.call() {
            Ok(response) => response,
            Err(error) => return drop(sender.send(Event::Failed(error.to_string()))),
        };
        let resumed = response.status() == 206;
        let length = response.body().content_length();
        let total = length.map(|len| if resumed { len + already } else { len });
        if sender.send(Event::Started { total, resumed }).is_err() {
            return;
        }
        let mut reader = response.body_mut().as_reader();
        let mut buffer = vec![0u8; 256 * 1024];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => return, // finished: dropping `sender` tells the main thread
                Ok(n) => {
                    if sender.send(Event::Data(buffer[..n].to_vec())).is_err() {
                        return; // main thread gave up on us
                    }
                }
                Err(error) => return drop(sender.send(Event::Failed(error.to_string()))),
            }
        }
    });

    let mut file = OpenOptions::new().create(true).append(true).open(dest)?;
    let mut written = already;
    let mut total = None;
    let mut last_percent = None;
    loop {
        match receiver.recv_timeout(STALL_TIMEOUT) {
            Ok(Event::Started { total: size, resumed }) => {
                if !resumed && already > 0 {
                    // Server can't continue a partial download: start over.
                    file.set_len(0)?;
                    written = 0;
                }
                total = size;
            }
            Ok(Event::Data(chunk)) => {
                file.write_all(&chunk)?;
                written += chunk.len() as u64;
                if let Some(total) = total.filter(|&t| t > 0) {
                    let percent = (written * 100 / total).min(100) as u8;
                    if last_percent != Some(percent) {
                        log(Msg::Progress(percent));
                        last_percent = Some(percent);
                    }
                }
            }
            Ok(Event::Failed(error)) => bail!("{error}"),
            Err(RecvTimeoutError::Timeout) => bail!("Download stalled"),
            Err(RecvTimeoutError::Disconnected) => break, // thread finished
        }
    }

    if let Some(total) = total
        && written < total
    {
        bail!("Connection closed early ({} of {})", human_size(written), human_size(total));
    }
    Ok(())
}
