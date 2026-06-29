use std::collections::HashMap;
use std::io::Read;
use std::sync::Arc;
use tokio::sync::Mutex;
use portable_pty::{CommandBuilder, MasterPty, PtySize};
use tauri::{AppHandle, Emitter};

pub struct PtySession {
    pub writer: Option<Box<dyn std::io::Write + Send + 'static>>,
    pub master_pty: Option<Box<dyn MasterPty + Send + 'static>>,
    pub child: Option<Box<dyn portable_pty::Child + Send + Sync + 'static>>,
    pub reader_handle: Option<std::thread::JoinHandle<()>>,
}

pub struct PtyManagerInner {
    sessions: HashMap<String, PtySession>,
    next_id: u64,
}

#[derive(Clone)]
pub struct PtyManager {
    inner: Arc<Mutex<PtyManagerInner>>,
}

impl PtyManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(PtyManagerInner {
                sessions: HashMap::new(),
                next_id: 1,
            })),
        }
    }

    pub async fn spawn(
        &self,
        app: AppHandle,
        shell: Option<String>,
        size: Option<(u16, u16)>,
    ) -> Result<String, String> {
        let pty_system = portable_pty::native_pty_system();
        let (cols, rows) = size.unwrap_or((80, 24));

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: cols * 8,
                pixel_height: rows * 16,
            })
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        let shell_cmd = shell.unwrap_or_else(|| {
            if cfg!(target_os = "windows") {
                "powershell.exe".to_string()
            } else {
                "bash".to_string()
            }
        });

        let mut cmd = CommandBuilder::new(&shell_cmd);
        if cfg!(target_os = "windows") {
            cmd.arg("-NoLogo");
        }
        cmd.env("TERM", "xterm-256color");

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn shell '{}': {}", shell_cmd, e))?;

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone PTY reader: {}", e))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to get PTY writer: {}", e))?;

        let master_pty = Some(pair.master);

        let mut inner = self.inner.lock().await;
        let term_id = format!("term-{}", inner.next_id);
        inner.next_id += 1;

        let app_clone = app.clone();
        let tid = term_id.clone();
        let reader_handle = std::thread::Builder::new()
            .name(format!("pty-reader-{}", tid))
            .spawn(move || {
                let mut buf = vec![0u8; 4096];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => {
                            let _ = app_clone.emit(
                                "terminal:exit",
                                serde_json::json!({
                                    "terminal_id": tid,
                                    "code": 0,
                                }),
                            );
                            break;
                        }
                        Ok(n) => {
                            if let Ok(data) = String::from_utf8(buf[..n].to_vec()) {
                                let _ = app_clone.emit(
                                    "terminal:output",
                                    serde_json::json!({
                                        "terminal_id": tid,
                                        "data": data,
                                    }),
                                );
                            }
                        }
                        Err(e) => {
                            log::error!("PTY read error for {}: {}", tid, e);
                            let _ = app_clone.emit(
                                "terminal:exit",
                                serde_json::json!({
                                    "terminal_id": tid,
                                    "code": -1,
                                }),
                            );
                            break;
                        }
                    }
                }
            })
            .map_err(|e| format!("Failed to spawn reader thread: {}", e))?;

        inner.sessions.insert(
            term_id.clone(),
            PtySession {
                writer: Some(writer),
                master_pty,
                child: Some(child),
                reader_handle: Some(reader_handle),
            },
        );

        Ok(term_id)
    }

    pub async fn write(&self, terminal_id: &str, data: &str) -> Result<(), String> {
        let mut inner = self.inner.lock().await;
        let session = inner
            .sessions
            .get_mut(terminal_id)
            .ok_or_else(|| format!("Terminal '{}' not found", terminal_id))?;

        let writer = session
            .writer
            .as_mut()
            .ok_or_else(|| "Writer already taken".to_string())?;

        writer
            .write_all(data.as_bytes())
            .map_err(|e| format!("Write error: {}", e))?;

        writer
            .flush()
            .map_err(|e| format!("Flush error: {}", e))?;

        Ok(())
    }

    pub async fn resize(&self, terminal_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let mut inner = self.inner.lock().await;
        let session = inner
            .sessions
            .get_mut(terminal_id)
            .ok_or_else(|| format!("Terminal '{}' not found", terminal_id))?;

        if let Some(ref master) = session.master_pty {
            let size = PtySize {
                rows,
                cols,
                pixel_width: cols * 8,
                pixel_height: rows * 16,
            };
            master
                .resize(size)
                .map_err(|e| format!("Resize error: {}", e))?;
        }

        Ok(())
    }

    pub async fn kill(&self, terminal_id: &str) -> Result<(), String> {
        let mut inner = self.inner.lock().await;
        let session = inner
            .sessions
            .remove(terminal_id)
            .ok_or_else(|| format!("Terminal '{}' not found", terminal_id))?;

        if let Some(mut child) = session.child {
            let _ = child.kill();
        }

        Ok(())
    }

    pub async fn list(&self) -> Vec<String> {
        let inner = self.inner.lock().await;
        inner.sessions.keys().cloned().collect()
    }
}
