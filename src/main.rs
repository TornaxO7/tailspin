mod highlighter;

use crate::highlighter::Highlighter;

use linemux::MuxedLines;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::NamedTempFile;
use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    let input = "example-logs/1.log";
    let line_count = count_lines(input).expect("Failed to count lines");
    let highlighter = Highlighter::new();

    let output_file = NamedTempFile::new().unwrap();
    let output_path = output_file.path().to_path_buf();
    let output_writer = BufWriter::new(output_file);

    let (tx, rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        tail_file(input, output_writer, highlighter, line_count, Some(tx))
            .await
            .expect("TODO: panic message");
    });

    // Wait for the signal from the other task before continuing
    rx.await.expect("Failed receiving from oneshot channel");

    open_file_with_less(output_path.to_str().unwrap());

    cleanup(output_path);
}

fn cleanup(output_path: PathBuf) {
    if let Err(err) = std::fs::remove_file(output_path) {
        eprintln!("Failed to remove the temporary file: {}", err);
    }
}

async fn tail_file<R>(
    path: &str,
    mut output_writer: BufWriter<R>,
    highlighter: Highlighter,
    line_count: usize,
    mut tx: Option<oneshot::Sender<()>>,
) -> io::Result<()>
where
    R: Write + Send + 'static,
{
    let mut lines = MuxedLines::new()?;
    let mut current_line = 1;
    lines.add_file_from_start(path).await?;

    while let Ok(Some(line)) = lines.next_line().await {
        if current_line == line_count {
            if let Some(tx) = tx.take() {
                tx.send(()).expect("Failed sending to oneshot channel");
            }
        }

        let highlighted_string = highlighter.apply(line.line());

        writeln!(output_writer, "{}", highlighted_string)?;
        output_writer.flush()?;
        current_line += 1;
    }

    Ok(())
}

fn open_file_with_less(path: &str) {
    let output = Command::new("less").arg(path).status();

    match output {
        Ok(status) => {
            if !status.success() {
                eprintln!("Failed to open file with less");
            }
        }
        Err(err) => {
            eprintln!("Failed to execute pager command: {}", err);
        }
    }
}

fn count_lines<P: AsRef<Path>>(file_path: P) -> io::Result<usize> {
    let file = File::open(file_path)?;
    let reader = io::BufReader::new(file);

    Ok(reader.lines().count())
}
