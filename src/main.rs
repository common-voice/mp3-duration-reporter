/// Iterates a directory for .mp3 files and records each duration and reports the total
///
/// This tool should be passed a path to a directory. It will create a file called "times.txt"
/// next to the given directory and search it for all files with the ".mp3" file extension.
///
/// For each found ".mp3", it will calculate its duration (in milliseconds) and add a line to
/// "times.txt" with the form "`path/to/file` = 1000".
///
/// Finally, it will output the total time of all ".mp3" files in the directory (in milliseconds).
///
/// This program uses non-blocking I/O for everything, so it should be able to handle a massive
/// number of files with relatively few threads.
use camino::Utf8PathBuf;
use std::convert::TryInto;
use tokio::io::AsyncWriteExt;
use tracing::Instrument;

/// The max number of (path, duration) pairs that can be queued up for writing to the
/// output file
const CHANNEL_LEN: usize = 1000000;

/// The name of the output file
const OUT_FILE_NAME: &str = "times.txt";

#[tokio::main]
async fn main() {
    // Controlled by environment. Use RUST_LOG
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let path = Utf8PathBuf::from_path_buf(std::path::PathBuf::from(
        std::env::args_os().nth(1).expect("expected directory path"),
    ))
    .expect("directory path not UTF-8");

    let out_path = path.parent().unwrap().join(OUT_FILE_NAME);

    tracing::info!("processing directory `{path}`");
    tracing::info!("output to file `{out_path}`");

    let (sender, mut receiver) = tokio::sync::mpsc::channel(CHANNEL_LEN);

    let sender_task = tokio::spawn(async move {
        let mut dir_entries = tokio::fs::read_dir(&path)
            .await
            .expect("path needs to exist");

        let mut mp3_tasks = Vec::new();

        while let Some(entry) = dir_entries
            .next_entry()
            .await
            .expect("an error occuring reading directory")
        {
            let path = Utf8PathBuf::from_path_buf(entry.path())
                .expect("directory contained non-UTF-8 file");

            if path.extension() == Some("mp3") {
                let file_span = tracing::info_span!("file_span", "`{path}`");
                let sender = sender.clone();

                mp3_tasks.push(tokio::spawn(
                    async move {
                        tracing::info!("reading mp3 file");

                        let mp3_bytes = tokio::fs::read(&path)
                            .await
                            .expect("failed to read mp3 file");

                        tracing::debug!("calculating duration");

                        let duration: u64 = match mp3_duration::from_read(&mut mp3_bytes.as_slice())
                        {
                            Ok(x) => x.as_millis().try_into().unwrap(),
                            Err(e) => {
                                tracing::error!("an error occurred on file `{path}`: {e}");
                                0
                            }
                        };

                        tracing::debug!("duration: {duration}");

                        sender.send((path, duration)).await.unwrap();
                    }
                    .instrument(file_span),
                ));
            } else {
                tracing::debug!("skipping file `{path}` (not an mp3)");
            }
        }

        for task in mp3_tasks.into_iter() {
            task.await.unwrap();
        }
    });

    let mut out_file = tokio::fs::File::create(&out_path)
        .await
        .expect("failed to create times file");

    let mut total = 0;

    while let Some((path, duration)) = receiver.recv().await {
        let line = format!("`{path}` = {duration}\n");

        tracing::debug!("writing \"`{path}` = {duration}\" to file");

        out_file
            .write_all(line.as_bytes())
            .await
            .expect("failed to write to time file");

        total += duration;
    }

    sender_task.await.unwrap();

    tracing::info!("total: {total}");
    println!("{total}");
}
