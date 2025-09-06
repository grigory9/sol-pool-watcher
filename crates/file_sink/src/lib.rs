use anyhow::Result;
use chrono::{Datelike, Utc};
use serde::Serialize;
use std::{collections::HashMap, path::PathBuf};
use tokio::{
    fs::OpenOptions,
    io::AsyncWriteExt,
    sync::mpsc,
    time::{interval, Duration},
};

#[derive(Clone, Debug)]
pub struct FileSinkCfg {
    pub dir: PathBuf,
    pub rotate_daily: bool,
}

#[derive(Clone)]
pub struct FileSink {
    tx: mpsc::Sender<(String, String)>,
}

impl FileSink {
    pub async fn new(cfg: FileSinkCfg) -> Result<Self> {
        tokio::fs::create_dir_all(&cfg.dir).await?;
        let (tx, mut rx) = mpsc::channel::<(String, String)>(4096);
        let dir = cfg.dir.clone();
        let rotate = cfg.rotate_daily;
        tokio::spawn(async move {
            let mut buffers: HashMap<PathBuf, Vec<String>> = HashMap::new();
            let mut ticker = interval(Duration::from_millis(700));
            loop {
                tokio::select! {
                    Some((stream, line)) = rx.recv() => {
                        let path = if rotate {
                            let now = Utc::now();
                            dir.join(format!("{}-{:04}-{:02}-{:02}.jsonl", stream, now.year(), now.month(), now.day()))
                        } else {
                            dir.join(format!("{}.jsonl", stream))
                        };
                        buffers.entry(path).or_default().push(line);
                    }
                    _ = ticker.tick() => {
                        if buffers.is_empty() && rx.is_closed() { break; }
                        if !buffers.is_empty() {
                            flush_buffers(&mut buffers).await;
                        }
                        if rx.is_closed() && buffers.is_empty() { break; }
                    }
                    else => {
                        if !buffers.is_empty() {
                            flush_buffers(&mut buffers).await;
                        }
                        break;
                    }
                }
            }
        });
        Ok(Self { tx })
    }

    pub async fn write_json<T: Serialize>(&self, stream: &str, value: &T) -> Result<()> {
        let line = serde_json::to_string(value)? + "\n";
        self.tx
            .send((stream.to_string(), line))
            .await
            .map_err(|_| anyhow::anyhow!("file sink closed"))
    }
}

async fn flush_buffers(buffers: &mut HashMap<PathBuf, Vec<String>>) {
    for (path, lines) in buffers.drain() {
        if let Err(e) = write_lines(&path, &lines).await {
            tracing::error!(?e, path=?path, "file sink write failed");
        }
    }
}

async fn write_lines(path: &PathBuf, lines: &[String]) -> Result<()> {
    if let Some(parent) = path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            tracing::error!(?e, path=?path, "create_dir_all failed");
            return Ok(()); // swallow
        }
    }
    match OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await
    {
        Ok(mut file) => {
            for l in lines {
                if let Err(e) = file.write_all(l.as_bytes()).await {
                    tracing::error!(?e, path=?path, "write_all failed");
                    return Ok(());
                }
            }
            if let Err(e) = file.flush().await {
                tracing::error!(?e, path=?path, "flush failed");
            }
        }
        Err(e) => {
            tracing::error!(?e, path=?path, "open failed");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_buffer_and_rotate() {
        let dir = tempdir().unwrap();
        let sink = FileSink::new(FileSinkCfg {
            dir: dir.path().into(),
            rotate_daily: true,
        })
        .await
        .unwrap();
        sink.write_json("stream", &serde_json::json!({"a":1}))
            .await
            .unwrap();
        sleep(Duration::from_millis(800)).await;
        let now = Utc::now();
        let path = dir.path().join(format!(
            "stream-{:04}-{:02}-{:02}.jsonl",
            now.year(),
            now.month(),
            now.day()
        ));
        let data = tokio::fs::read_to_string(path).await.unwrap();
        assert!(data.contains("\"a\":1"));
    }

    #[tokio::test]
    async fn test_write_failure() {
        let dir = tempdir().unwrap();
        // create directory with same name as target file to induce failure
        std::fs::create_dir(dir.path().join("bad.jsonl")).unwrap();
        let sink = FileSink::new(FileSinkCfg {
            dir: dir.path().into(),
            rotate_daily: false,
        })
        .await
        .unwrap();
        sink.write_json("bad", &serde_json::json!({"b":2}))
            .await
            .unwrap();
        sleep(Duration::from_millis(800)).await;
        // ensure it is still a directory, meaning no file written
        assert!(dir.path().join("bad.jsonl").is_dir());
    }
}
