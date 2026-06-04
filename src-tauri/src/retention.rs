use anyhow::Result;
use std::{fs, path::Path};

pub fn clear_recording_files(recordings_dir: &Path) -> Result<usize> {
    if !recordings_dir.exists() {
        return Ok(0);
    }
    let mut removed = 0;
    for entry in fs::read_dir(recordings_dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_file() && is_recording_file(&path) {
            fs::remove_file(path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

fn is_recording_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "wav" | "flac" | "m4a" | "mp3" | "ogg"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clears_recording_files_only() {
        let temp = tempfile::tempdir().unwrap();
        let recordings = temp.path().join("recordings");
        fs::create_dir_all(&recordings).unwrap();
        fs::write(recordings.join("a.wav"), "audio").unwrap();
        fs::write(recordings.join("b.MP3"), "audio").unwrap();
        fs::write(recordings.join("notes.txt"), "keep").unwrap();
        fs::create_dir_all(recordings.join("nested")).unwrap();
        fs::write(recordings.join("nested").join("inner.wav"), "keep").unwrap();

        let removed = clear_recording_files(&recordings).unwrap();
        assert_eq!(removed, 2);
        assert!(!recordings.join("a.wav").exists());
        assert!(!recordings.join("b.MP3").exists());
        assert!(recordings.join("notes.txt").exists());
        assert!(recordings.join("nested").join("inner.wav").exists());
    }

    #[test]
    fn missing_recordings_dir_is_empty() {
        let temp = tempfile::tempdir().unwrap();
        let removed = clear_recording_files(&temp.path().join("missing")).unwrap();
        assert_eq!(removed, 0);
    }
}
