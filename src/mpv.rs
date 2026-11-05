use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct State {
    pub position: u64,
    pub duration: u64,
    pub paused: bool,
    pub idle: bool,
    pub volume: u8,
    pub failed: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            position: 0,
            duration: 0,
            paused: false,
            idle: true,
            volume: 0,
            failed: false,
        }
    }
}

pub fn command_json(command: &[Value]) -> String {
    format!("{}\n", json!({ "command": command }))
}

fn mpv_binary() -> PathBuf {
    std::env::var_os("VLSWMPLAYER_MPV")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("mpv"))
}

fn socket_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    dir.join(format!("vlswmplayer-mpv-{}.sock", std::process::id()))
}

pub struct Player {
    child: Child,
    socket: PathBuf,
    writer: UnixStream,
    shared: Arc<Mutex<State>>,
    loaded: Arc<AtomicBool>,
}

impl Player {
    pub fn new(volume: u8) -> Result<Self, String> {
        let socket = socket_path();
        let _ = std::fs::remove_file(&socket);

        let mut child = Command::new(mpv_binary())
            .args([
                "--no-video",
                "--vo=null",
                "--force-window=no",
                "--audio-display=no",
                "--idle=yes",
                "--msg-level=all=error",
                "--no-terminal",
                "--no-input-terminal",
            ])
            .arg(format!("--input-ipc-server={}", socket.display()))
            .arg(format!("--volume={volume}"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("mpv: {e}"))?;

        let deadline = Instant::now() + Duration::from_secs(4);
        let stream = loop {
            if let Ok(stream) = UnixStream::connect(&socket) {
                break stream;
            }
            if let Ok(Some(status)) = child.try_wait() {
                use std::io::Read;
                let mut err = String::new();
                if let Some(mut pipe) = child.stderr.take() {
                    let _ = pipe.read_to_string(&mut err);
                }
                return Err(format!("mpv exited {status}: {}", err.trim()));
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                return Err("mpv: ipc socket did not appear".to_owned());
            }
            std::thread::sleep(Duration::from_millis(50));
        };

        let writer = stream.try_clone().map_err(|e| e.to_string())?;
        let shared = Arc::new(Mutex::new(State {
            volume,
            idle: true,
            ..Default::default()
        }));
        let loaded = Arc::new(AtomicBool::new(false));

        let reader_state = Arc::clone(&shared);
        let reader_loaded = Arc::clone(&loaded);
        std::thread::spawn(move || {
            let reader = BufReader::new(stream);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                let Ok(value) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                let mut state = reader_state.lock().unwrap();
                match value.get("event").and_then(Value::as_str) {
                    Some("property-change") => {
                        let name = value.get("name").and_then(Value::as_str).unwrap_or("");
                        let data = &value["data"];
                        match name {
                            "time-pos" => {
                                if let Some(secs) = data.as_f64() {
                                    state.position = secs.max(0.0) as u64;
                                }
                            }
                            "duration" => {
                                if let Some(secs) = data.as_f64() {
                                    if secs > 0.0 {
                                        state.duration = secs as u64;
                                    }
                                }
                            }
                            "pause" => {
                                if let Some(paused) = data.as_bool() {
                                    state.paused = paused;
                                }
                            }
                            "idle-active" => {
                                if let Some(idle) = data.as_bool() {
                                    state.idle = idle;
                                }
                            }
                            "volume" => {
                                if let Some(vol) = data.as_f64() {
                                    state.volume = vol.clamp(0.0, 100.0) as u8;
                                }
                            }
                            _ => {}
                        }
                    }
                    Some("file-loaded") => {
                        state.idle = false;
                        reader_loaded.store(true, Ordering::Relaxed);
                    }
                    Some("end-file") => {
                        let failed = value.get("reason").and_then(Value::as_str) == Some("error");
                        state.failed = failed;
                        state.idle = true;
                        if !failed {
                            state.position = state.duration;
                        }
                    }
                    _ => {}
                }
            }
        });

        let player = Self {
            child,
            socket,
            writer,
            shared,
            loaded,
        };

        for (id, name) in [
            (1, "time-pos"),
            (2, "duration"),
            (3, "pause"),
            (4, "idle-active"),
            (5, "volume"),
            (6, "eof-reached"),
        ] {
            player.send(vec![
                "observe_property".into(),
                id.into(),
                name.into(),
            ]);
        }

        Ok(player)
    }

    fn send(&self, command: Vec<Value>) {
        let mut writer = &self.writer;
        let _ = writer.write_all(command_json(&command).as_bytes());
    }

    pub fn load(&mut self, url: &str) {
        self.loaded.store(false, Ordering::Relaxed);
        {
            let mut state = self.shared.lock().unwrap();
            state.idle = false;
            state.position = 0;
            state.duration = 0;
            state.failed = false;
        }
        self.send(vec!["loadfile".into(), url.into(), "replace".into()]);
        self.send(vec!["set".into(), "pause".into(), "no".into()]);
    }

    pub fn toggle(&mut self) {
        self.send(vec!["cycle".into(), "pause".into()]);
        let mut state = self.shared.lock().unwrap();
        state.paused = !state.paused;
    }

    pub fn seek(&mut self, secs: u64) {
        self.send(vec![
            "seek".into(),
            f64::from(secs as u32).into(),
            "absolute".into(),
        ]);
        self.shared.lock().unwrap().position = secs;
    }

    pub fn set_volume(&mut self, volume: u8) {
        let volume = volume.min(100);
        {
            self.shared.lock().unwrap().volume = volume;
        }
        self.send(vec!["set".into(), "volume".into(), volume.into()]);
    }

    pub fn state(&self) -> State {
        self.shared.lock().unwrap().clone()
    }

    pub fn position(&self) -> u64 {
        self.shared.lock().unwrap().position
    }

    pub fn duration(&self) -> u64 {
        self.shared.lock().unwrap().duration
    }

    pub fn volume(&self) -> u8 {
        self.shared.lock().unwrap().volume
    }

    pub fn finished(&self) -> bool {
        self.loaded.load(Ordering::Relaxed) && self.shared.lock().unwrap().idle
    }

    pub fn take_failed(&mut self) -> bool {
        let mut state = self.shared.lock().unwrap();
        let failed = state.failed;
        state.failed = false;
        failed
    }

    pub fn stop(&mut self) {
        self.send(vec!["stop".into()]);
        let mut state = self.shared.lock().unwrap();
        state.idle = true;
        state.position = 0;
        state.duration = 0;
    }

    pub fn quit(&mut self) {
        self.send(vec!["quit".into()]);
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_command_line() {
        let line = command_json(&["loadfile".into(), "http://x".into()]);
        let value: Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(value["command"][0], "loadfile");
        assert_eq!(value["command"][1], "http://x");
    }

    #[test]
    fn state_starts_idle() {
        let state = State::default();
        assert!(state.idle);
        assert_eq!(state.position, 0);
        assert_eq!(state.duration, 0);
    }

#[test]
    #[ignore]
    fn manual_mpv_plays_stream() {
        let cid = crate::sc::client_id().expect("client id");
        let track = crate::sc::api_track("2260087832", &cid).expect("track");
        let url = crate::sc::resolve_stream(&track, &cid).expect("stream");
        let mut player = Player::new(20).expect("mpv");
        player.load(&url);
        for _ in 0..12 {
            std::thread::sleep(Duration::from_millis(700));
            if player.position() > 0 {
                break;
            }
        }
        assert!(player.position() >= 1, "position {:?}", player.state());
        player.quit();
    }
}
