//! Reads Path of Exile's `Client.txt` log.
//!
//! We only ever *read* the file. Nothing here touches the game client, its memory or
//! its data files.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// How much of the end of the log we scan on start-up to find the current area.
const STARTUP_SCAN_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogEvent {
    /// `Generating level 83 area "MapWorldsStrand" with seed 1`. Written when a zone
    /// starts loading. The area id is not localised, so it works for every client language.
    AreaGenerated { level: u32, area_id: String },
    /// `: You have entered Lioneye's Watch.` Localised, so only used for display and as a
    /// fallback for user-defined area names.
    AreaEntered { name: String },
}

pub fn parse_line(line: &str) -> Option<LogEvent> {
    let line = line.trim_end_matches(['\r', '\n']);

    if let Some(idx) = line.find("Generating level ") {
        let rest = &line[idx + "Generating level ".len()..];
        let (level, rest) = rest.split_once(' ')?;
        let level = level.parse().ok()?;
        let rest = rest.strip_prefix("area \"")?;
        let (area_id, _) = rest.split_once('"')?;
        return Some(LogEvent::AreaGenerated {
            level,
            area_id: area_id.to_string(),
        });
    }

    if let Some(idx) = line.find("] : You have entered ") {
        let name = line[idx + "] : You have entered ".len()..].trim_end_matches('.');
        if !name.is_empty() {
            return Some(LogEvent::AreaEntered {
                name: name.to_string(),
            });
        }
    }

    None
}

/// Follows a growing log file, like `tail -f`.
pub struct LogTail {
    path: PathBuf,
    pos: u64,
    partial: Vec<u8>,
}

impl LogTail {
    /// Opens the log and returns the events found near its end, so the caller knows
    /// which area the player is in right now. Later calls to [`LogTail::poll`] only
    /// return lines written after this.
    pub fn open(path: &Path) -> std::io::Result<(Self, Vec<LogEvent>)> {
        let mut file = File::open(path)?;
        let len = file.metadata()?.len();
        let start = len.saturating_sub(STARTUP_SCAN_BYTES);
        file.seek(SeekFrom::Start(start))?;
        let mut buf = Vec::with_capacity((len - start) as usize);
        file.read_to_end(&mut buf)?;

        let mut tail = LogTail {
            path: path.to_path_buf(),
            pos: start,
            partial: Vec::new(),
        };
        // When starting mid-file the first line is probably cut off; drop it.
        if start > 0 {
            match buf.iter().position(|&b| b == b'\n') {
                Some(nl) => {
                    tail.pos += nl as u64 + 1;
                    buf.drain(..=nl);
                }
                None => {
                    tail.pos = len;
                    buf.clear();
                }
            }
        }
        let events = tail.consume(&buf);
        Ok((tail, events))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns events from lines appended since the last call.
    pub fn poll(&mut self) -> std::io::Result<Vec<LogEvent>> {
        let mut file = File::open(&self.path)?;
        let len = file.metadata()?.len();
        if len < self.pos {
            // The log was truncated or replaced (the game can do this when it gets big).
            self.pos = 0;
            self.partial.clear();
        }
        if len == self.pos {
            return Ok(Vec::new());
        }
        file.seek(SeekFrom::Start(self.pos))?;
        let mut buf = Vec::with_capacity((len - self.pos) as usize);
        file.read_to_end(&mut buf)?;
        Ok(self.consume(&buf))
    }

    fn consume(&mut self, buf: &[u8]) -> Vec<LogEvent> {
        self.pos += buf.len() as u64;
        self.partial.extend_from_slice(buf);
        let Some(last_nl) = self.partial.iter().rposition(|&b| b == b'\n') else {
            return Vec::new();
        };
        let complete: Vec<u8> = self.partial.drain(..=last_nl).collect();
        String::from_utf8_lossy(&complete)
            .lines()
            .filter_map(parse_line)
            .collect()
    }
}

/// Where `Client.txt` usually lives. The game writes it to `logs\Client.txt` next to
/// its executable, so a running game's own path is the best guess.
pub fn candidate_paths(game_exe: Option<&Path>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(dir) = game_exe.and_then(Path::parent) {
        paths.push(dir.join("logs").join("Client.txt"));
    }
    for base in [
        r"C:\Program Files (x86)\Grinding Gear Games\Path of Exile",
        r"C:\Program Files\Grinding Gear Games\Path of Exile",
        r"C:\Program Files (x86)\Steam\steamapps\common\Path of Exile",
        r"C:\Program Files\Steam\steamapps\common\Path of Exile",
        r"C:\Program Files\Epic Games\PathOfExile",
    ] {
        paths.push(Path::new(base).join("logs").join("Client.txt"));
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_area_generated() {
        let line = r#"2026/11/14 08:00:00 123456 cff945b9 [DEBUG Client 1234] Generating level 1 area "1_1_town" with seed 1"#;
        assert_eq!(
            parse_line(line),
            Some(LogEvent::AreaGenerated {
                level: 1,
                area_id: "1_1_town".into()
            })
        );
    }

    #[test]
    fn parses_area_entered() {
        let line = "2026/11/14 08:00:01 123457 cb7f2f6a [INFO Client 1234] : You have entered Lioneye's Watch.\r\n";
        assert_eq!(
            parse_line(line),
            Some(LogEvent::AreaEntered {
                name: "Lioneye's Watch".into()
            })
        );
    }

    #[test]
    fn ignores_chat_that_looks_like_an_area_line() {
        // A player typing the text in chat must not count: chat lines have "Name: text"
        // after the bracket, not "] : You have entered".
        let line = "2026/11/14 08:00:01 1 2 [INFO Client 1234] #Troll: You have entered Highgate.";
        assert_eq!(parse_line(line), None);
    }

    #[test]
    fn tails_appended_lines_and_partial_writes() {
        let dir = std::env::temp_dir().join(format!("poe-tool-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Client.txt");
        std::fs::write(
            &path,
            "x [DEBUG Client 1] Generating level 68 area \"MapWorldsStrand\" with seed 1\n",
        )
        .unwrap();

        let (mut tail, initial) = LogTail::open(&path).unwrap();
        assert_eq!(initial.len(), 1);

        let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
        write!(f, "x [DEBUG Client 1] Generating level 1 area \"1_1_").unwrap();
        f.flush().unwrap();
        assert!(tail.poll().unwrap().is_empty());
        writeln!(f, "town\" with seed 2").unwrap();
        f.flush().unwrap();
        assert_eq!(
            tail.poll().unwrap(),
            vec![LogEvent::AreaGenerated {
                level: 1,
                area_id: "1_1_town".into()
            }]
        );

        // Truncation starts over from the beginning.
        std::fs::write(&path, "x [INFO Client 1] : You have entered Highgate.\n").unwrap();
        assert_eq!(
            tail.poll().unwrap(),
            vec![LogEvent::AreaEntered {
                name: "Highgate".into()
            }]
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
