use lexiclean::Lexiclean;
use path_slash::PathBufExt;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::thread;

pub(super) fn clean_path_string(p: &PathBuf) -> String {
    p.clone()
        .lexiclean()
        .to_slash()
        .unwrap()
        .to_string()
        .replace("\\", "/")
}

pub(super) fn tee_to_console_and_log<R: Read + Send + 'static>(
    mut reader: R,
    mut log: File,
    mut console: impl Write + Send + 'static,
) {
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let _ = console.write_all(&buf[..n]);
                    let _ = console.flush();
                    let _ = log.write_all(&buf[..n]);
                    let _ = log.flush();
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::tee_to_console_and_log;
    use std::fs::{self, File};
    use std::io::Cursor;
    use std::time::{Duration, Instant};

    #[test]
    fn tee_writes_log_and_console() {
        let dir = std::env::temp_dir().join(format!(
            "foil-tee-{}-{}",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let log_path = dir.join("renderer.log");
        let console_path = dir.join("console.txt");
        let log = File::create(&log_path).unwrap();
        let console = File::create(&console_path).unwrap();

        tee_to_console_and_log(Cursor::new(b"react boom\n".to_vec()), log, console);

        let deadline = Instant::now() + Duration::from_secs(2);
        let mut logged = String::new();
        while Instant::now() < deadline {
            logged = fs::read_to_string(&log_path).unwrap_or_default();
            if logged.contains("react boom") {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let consoled = fs::read_to_string(&console_path).unwrap();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(logged, "react boom\n");
        assert_eq!(consoled, "react boom\n");
    }
}
