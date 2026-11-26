//! Asynchronous Pipelined Disk Writer
//! Moves all disk writes completely off the async Tokio network receive path.
//! Workers submit zero-copy `bytes::Bytes` chunks gated by a bounded byte-budget
//! `Semaphore`; a dedicated OS thread coalesces adjacent/overlapping ranges in a
//! `BTreeMap<u64, Bytes>` and issues positional writes.

use bytes::Bytes;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot, OwnedSemaphorePermit, Semaphore};

const DEFAULT_MAX_BUFFERED_BYTES: usize = 64 * 1024 * 1024; // 64 MiB
const STAGING_FLUSH_THRESHOLD_BYTES: usize = 2 * 1024 * 1024; // 2 MiB
const PERIODIC_SYNC_INTERVAL: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub struct WriteChunk {
    pub offset: u64,
    pub data: Bytes,
}

#[derive(Debug, Clone)]
struct StoredIoError {
    raw_os_error: Option<i32>,
    kind: io::ErrorKind,
    message: String,
}

impl StoredIoError {
    fn from_io(err: &io::Error) -> Self {
        Self {
            raw_os_error: err.raw_os_error(),
            kind: err.kind(),
            message: err.to_string(),
        }
    }

    fn to_io_error(&self) -> io::Error {
        if let Some(code) = self.raw_os_error {
            io::Error::from_raw_os_error(code)
        } else {
            io::Error::new(self.kind, self.message.clone())
        }
    }
}

enum WriterCommand {
    Write {
        offset: u64,
        data: Bytes,
        _permit: Option<OwnedSemaphorePermit>,
    },
    Flush {
        ack: oneshot::Sender<io::Result<()>>,
    },
    Shutdown,
}

/// Cloneable async handle used by workers to push positional writes with bounded memory backpressure.
#[derive(Clone)]
pub struct PipelinedWriterTx {
    sender: mpsc::Sender<WriterCommand>,
    byte_budget: Arc<Semaphore>,
    max_buffered_bytes: usize,
    cancelled: Arc<AtomicBool>,
    shared_error: Arc<Mutex<Option<StoredIoError>>>,
}

impl PipelinedWriterTx {
    fn check_alive(&self) -> io::Result<()> {
        if let Some(err) = self.shared_error.lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
            return Err(err.to_io_error());
        }
        if self.cancelled.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "pipeline writer aborted",
            ));
        }
        Ok(())
    }

    pub async fn write_chunk(&self, chunk: WriteChunk) -> io::Result<()> {
        self.write_bytes_at(chunk.offset, chunk.data).await
    }

    pub async fn write_bytes_at(&self, offset: u64, data: Bytes) -> io::Result<()> {
        self.check_alive()?;
        if data.is_empty() {
            return Ok(());
        }

        let permit_count = data.len().min(self.max_buffered_bytes).min(u32::MAX as usize) as u32;
        let permit = self
            .byte_budget
            .clone()
            .acquire_many_owned(permit_count)
            .await
            .map_err(|_| {
                self.check_alive().err().unwrap_or_else(|| {
                    io::Error::new(io::ErrorKind::BrokenPipe, "pipeline writer closed")
                })
            })?;

        self.check_alive()?;

        self.sender
            .send(WriterCommand::Write {
                offset,
                data,
                _permit: Some(permit),
            })
            .await
            .map_err(|_| {
                self.check_alive().err().unwrap_or_else(|| {
                    io::Error::new(io::ErrorKind::BrokenPipe, "pipeline writer channel closed")
                })
            })
    }

    pub async fn write_at(&self, offset: u64, data: Vec<u8>) -> io::Result<()> {
        self.write_bytes_at(offset, Bytes::from(data)).await
    }

    pub async fn flush(&self) -> io::Result<()> {
        self.check_alive()?;
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(WriterCommand::Flush { ack: tx })
            .await
            .map_err(|_| {
                self.check_alive().err().unwrap_or_else(|| {
                    io::Error::new(io::ErrorKind::BrokenPipe, "pipeline writer channel closed")
                })
            })?;
        match rx.await {
            Ok(res) => res,
            Err(_) => Err(self.check_alive().err().unwrap_or_else(|| {
                io::Error::new(io::ErrorKind::BrokenPipe, "pipeline writer stopped before flush ack")
            })),
        }
    }
}

pub struct PipelinedWriterHandle {
    tx: PipelinedWriterTx,
    sender: Option<mpsc::Sender<WriterCommand>>,
    cancelled: Arc<AtomicBool>,
    byte_budget: Arc<Semaphore>,
    shared_error: Arc<Mutex<Option<StoredIoError>>>,
    coalesced_write_ops: Arc<AtomicU64>,
    part_path: PathBuf,
    final_path: PathBuf,
    writer_thread: Option<std::thread::JoinHandle<io::Result<()>>>,
}

impl PipelinedWriterHandle {
    pub fn new<P: AsRef<Path>>(
        target_dir: P,
        filename: &str,
        total_size: Option<u64>,
        channel_capacity: usize,
    ) -> io::Result<Self> {
        Self::with_byte_budget(
            target_dir,
            filename,
            total_size,
            channel_capacity,
            DEFAULT_MAX_BUFFERED_BYTES,
        )
    }

    pub fn with_byte_budget<P: AsRef<Path>>(
        target_dir: P,
        filename: &str,
        total_size: Option<u64>,
        channel_capacity: usize,
        max_buffered_bytes: usize,
    ) -> io::Result<Self> {
        let target_ref = target_dir.as_ref();
        if !target_ref.exists() {
            std::fs::create_dir_all(target_ref)?;
        }
        let final_path = target_ref.join(filename);
        let part_path = target_ref.join(format!("{}.edmpart", filename));

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&part_path)?;

        // Pre-allocate using NTFS sparse allocation on Windows to avoid synchronous zero-fill stalls.
        if let Some(size) = total_size {
            #[cfg(windows)]
            {
                let _ = set_windows_sparse(&file, &part_path);
            }
            file.set_len(size)?;
        }

        let max_buffered_bytes = max_buffered_bytes.max(64 * 1024);
        let channel_capacity = channel_capacity.max(16);
        let (sender, mut receiver) = mpsc::channel::<WriterCommand>(channel_capacity);
        let byte_budget = Arc::new(Semaphore::new(max_buffered_bytes));
        let cancelled = Arc::new(AtomicBool::new(false));
        let shared_error: Arc<Mutex<Option<StoredIoError>>> = Arc::new(Mutex::new(None));
        let coalesced_write_ops = Arc::new(AtomicU64::new(0));

        let tx = PipelinedWriterTx {
            sender: sender.clone(),
            byte_budget: byte_budget.clone(),
            max_buffered_bytes,
            cancelled: cancelled.clone(),
            shared_error: shared_error.clone(),
        };

        let thread_cancelled = cancelled.clone();
        let thread_budget = byte_budget.clone();
        let thread_error = shared_error.clone();
        let thread_ops = coalesced_write_ops.clone();

        let writer_thread = std::thread::Builder::new()
            .name("edm-pipeline-writer".into())
            .spawn(move || -> io::Result<()> {
                let res = writer_loop(
                    file,
                    &mut receiver,
                    &thread_cancelled,
                    &thread_ops,
                );
                if let Err(ref err) = res {
                    let mut guard = thread_error.lock().unwrap_or_else(|p| p.into_inner());
                    if guard.is_none() {
                        *guard = Some(StoredIoError::from_io(err));
                    }
                }
                thread_budget.close();
                res
            })?;

        Ok(Self {
            tx,
            sender: Some(sender),
            cancelled,
            byte_budget,
            shared_error,
            coalesced_write_ops,
            part_path,
            final_path,
            writer_thread: Some(writer_thread),
        })
    }

    /// Returns a cloneable async writer handle for download workers.
    pub fn writer(&self) -> PipelinedWriterTx {
        self.tx.clone()
    }

    pub async fn write_chunk(&self, chunk: WriteChunk) -> io::Result<()> {
        self.tx.write_chunk(chunk).await
    }

    pub async fn write_bytes_at(&self, offset: u64, data: Bytes) -> io::Result<()> {
        self.tx.write_bytes_at(offset, data).await
    }

    pub async fn write_at(&self, offset: u64, data: Vec<u8>) -> io::Result<()> {
        self.tx.write_at(offset, data).await
    }

    pub async fn flush(&self) -> io::Result<()> {
        self.tx.flush().await
    }

    /// Number of coalesced contiguous OS write operations executed so far.
    pub fn coalesced_write_ops(&self) -> u64 {
        self.coalesced_write_ops.load(Ordering::Relaxed)
    }

    pub fn part_path(&self) -> &Path {
        &self.part_path
    }

    pub fn final_path(&self) -> &Path {
        &self.final_path
    }

    pub fn finish(mut self, referer_or_url: Option<&str>) -> io::Result<PathBuf> {
        if let Some(sender) = self.sender.take() {
            let _ = sender.try_send(WriterCommand::Shutdown);
            drop(sender);
        }

        if let Some(handle) = self.writer_thread.take() {
            join_writer_thread(handle)?;
        }

        if let Some(err) = self
            .shared_error
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
        {
            return Err(err.to_io_error());
        }

        if self.cancelled.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "pipeline writer aborted before finish",
            ));
        }

        // Apply atomic rename
        if self.final_path.exists() {
            let _ = std::fs::remove_file(&self.final_path);
        }
        std::fs::rename(&self.part_path, &self.final_path)?;

        // Attach Mark-of-the-Web (ZoneId=3) on Windows
        #[cfg(windows)]
        if let Some(source_url) = referer_or_url {
            use std::io::Write;
            let zone_path = format!("{}:Zone.Identifier", self.final_path.display());
            if let Ok(mut zone_file) = std::fs::File::create(&zone_path) {
                let _ = writeln!(
                    zone_file,
                    "[ZoneTransfer]\r\nZoneId=3\r\nHostUrl={}",
                    source_url
                );
            }
        }

        Ok(self.final_path.clone())
    }

    /// Cancels pending buffered writes without renaming `.edmpart` and propagates any thread panic or IO error.
    pub fn abort(&mut self) -> io::Result<()> {
        self.cancelled.store(true, Ordering::Release);
        self.byte_budget.close();
        if let Some(sender) = self.sender.take() {
            let _ = sender.try_send(WriterCommand::Shutdown);
            drop(sender);
        }
        if let Some(handle) = self.writer_thread.take() {
            join_writer_thread(handle)?;
        }
        if let Some(err) = self
            .shared_error
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
        {
            return Err(err.to_io_error());
        }
        Ok(())
    }
}

impl Drop for PipelinedWriterHandle {
    fn drop(&mut self) {
        if self.writer_thread.is_some() {
            self.cancelled.store(true, Ordering::Release);
            self.byte_budget.close();
            if let Some(sender) = self.sender.take() {
                let _ = sender.try_send(WriterCommand::Shutdown);
                drop(sender);
            }
            if let Some(handle) = self.writer_thread.take() {
                let _ = handle.join();
            }
        }
    }
}

fn join_writer_thread(handle: std::thread::JoinHandle<io::Result<()>>) -> io::Result<()> {
    match handle.join() {
        Ok(res) => res,
        Err(panic_payload) => {
            let msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                (*s).to_string()
            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("pipeline writer thread panicked: {}", msg),
            ))
        }
    }
}

#[cfg(windows)]
fn set_windows_sparse(file: &File, part_path: &Path) -> bool {
    use std::os::windows::io::AsRawHandle;
    const FSCTL_SET_SPARSE: u32 = 0x000900C4;
    extern "system" {
        fn DeviceIoControl(
            hDevice: isize,
            dwIoControlCode: u32,
            lpInBuffer: *const std::ffi::c_void,
            nInBufferSize: u32,
            lpOutBuffer: *mut std::ffi::c_void,
            nOutBufferSize: u32,
            lpBytesReturned: *mut u32,
            lpOverlapped: *mut std::ffi::c_void,
        ) -> i32;
    }

    let handle = file.as_raw_handle();
    let mut bytes_returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle as isize,
            FSCTL_SET_SPARSE,
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            0,
            &mut bytes_returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        tracing::warn!(
            "FSCTL_SET_SPARSE failed on {:?} ({}); continuing with standard set_len",
            part_path,
            io::Error::last_os_error()
        );
        return false;
    }
    true
}

fn apply_writer_cmd(
    cmd: WriterCommand,
    file: &File,
    staging: &mut BTreeMap<u64, Bytes>,
    staging_bytes: &mut usize,
    scratch: &mut Vec<u8>,
    permits: &mut Vec<OwnedSemaphorePermit>,
    coalesced_ops: &AtomicU64,
    pending_flushes: &mut Vec<oneshot::Sender<io::Result<()>>>,
    shutdown_requested: &mut bool,
) -> io::Result<()> {
    match cmd {
        WriterCommand::Write {
            offset,
            data,
            _permit,
        } => {
            if !data.is_empty() {
                // Guard against duplicate exact-offset writes overwriting longer staged slices.
                if staging.contains_key(&offset) {
                    flush_staging(file, staging, staging_bytes, scratch, permits, coalesced_ops)?;
                }
                *staging_bytes = staging_bytes.saturating_add(data.len());
                staging.insert(offset, data);
            }
            if let Some(p) = _permit {
                permits.push(p);
            }
        }
        WriterCommand::Flush { ack } => {
            pending_flushes.push(ack);
        }
        WriterCommand::Shutdown => {
            *shutdown_requested = true;
        }
    }
    Ok(())
}

fn fail_pending_flushes(pending_flushes: Vec<oneshot::Sender<io::Result<()>>>, err: &io::Error) {
    for ack in pending_flushes {
        let _ = ack.send(Err(StoredIoError::from_io(err).to_io_error()));
    }
}

fn writer_loop(
    file: File,
    receiver: &mut mpsc::Receiver<WriterCommand>,
    cancelled: &AtomicBool,
    coalesced_ops: &AtomicU64,
) -> io::Result<()> {
    let mut staging: BTreeMap<u64, Bytes> = BTreeMap::new();
    let mut staging_bytes: usize = 0;
    let mut permits: Vec<OwnedSemaphorePermit> = Vec::new();
    let mut scratch: Vec<u8> = Vec::new();
    let mut last_sync = Instant::now();

    'outer: while let Some(first_cmd) = receiver.blocking_recv() {
        if cancelled.load(Ordering::Acquire) {
            staging.clear();
            permits.clear();
            return Ok(());
        }

        let mut pending_flushes: Vec<oneshot::Sender<io::Result<()>>> = Vec::new();
        let mut shutdown_requested = false;

        if let Err(err) = apply_writer_cmd(
            first_cmd,
            &file,
            &mut staging,
            &mut staging_bytes,
            &mut scratch,
            &mut permits,
            coalesced_ops,
            &mut pending_flushes,
            &mut shutdown_requested,
        ) {
            fail_pending_flushes(pending_flushes, &err);
            return Err(err);
        }

        // Drain immediately queued commands to coalesce contiguous ranges across concurrent workers.
        while !shutdown_requested && staging_bytes < STAGING_FLUSH_THRESHOLD_BYTES {
            if cancelled.load(Ordering::Acquire) {
                staging.clear();
                permits.clear();
                let abort_err = io::Error::new(io::ErrorKind::Interrupted, "pipeline writer aborted");
                fail_pending_flushes(pending_flushes, &abort_err);
                return Ok(());
            }

            match receiver.try_recv() {
                Ok(cmd) => {
                    if let Err(err) = apply_writer_cmd(
                        cmd,
                        &file,
                        &mut staging,
                        &mut staging_bytes,
                        &mut scratch,
                        &mut permits,
                        coalesced_ops,
                        &mut pending_flushes,
                        &mut shutdown_requested,
                    ) {
                        fail_pending_flushes(pending_flushes, &err);
                        return Err(err);
                    }
                }
                Err(_) => break,
            }
        }

        if cancelled.load(Ordering::Acquire) {
            staging.clear();
            permits.clear();
            let abort_err = io::Error::new(io::ErrorKind::Interrupted, "pipeline writer aborted");
            fail_pending_flushes(pending_flushes, &abort_err);
            return Ok(());
        }

        if let Err(err) = flush_staging(
            &file,
            &mut staging,
            &mut staging_bytes,
            &mut scratch,
            &mut permits,
            coalesced_ops,
        ) {
            fail_pending_flushes(pending_flushes, &err);
            return Err(err);
        }

        if !pending_flushes.is_empty() {
            if let Err(err) = file.sync_data() {
                fail_pending_flushes(pending_flushes, &err);
                return Err(err);
            }
            last_sync = Instant::now();
            for ack in pending_flushes {
                let _ = ack.send(Ok(()));
            }
        } else if last_sync.elapsed() >= PERIODIC_SYNC_INTERVAL {
            let _ = file.sync_data();
            last_sync = Instant::now();
        }

        if shutdown_requested {
            break 'outer;
        }
    }

    if cancelled.load(Ordering::Acquire) {
        staging.clear();
        permits.clear();
        return Ok(());
    }

    // Drain any residual messages left in channel before final sync
    let mut final_flushes: Vec<oneshot::Sender<io::Result<()>>> = Vec::new();
    let mut ignored_shutdown = false;
    while let Ok(cmd) = receiver.try_recv() {
        if let Err(err) = apply_writer_cmd(
            cmd,
            &file,
            &mut staging,
            &mut staging_bytes,
            &mut scratch,
            &mut permits,
            coalesced_ops,
            &mut final_flushes,
            &mut ignored_shutdown,
        ) {
            fail_pending_flushes(final_flushes, &err);
            return Err(err);
        }
    }

    if let Err(err) = flush_staging(
        &file,
        &mut staging,
        &mut staging_bytes,
        &mut scratch,
        &mut permits,
        coalesced_ops,
    ) {
        fail_pending_flushes(final_flushes, &err);
        return Err(err);
    }
    if let Err(err) = file.sync_data() {
        fail_pending_flushes(final_flushes, &err);
        return Err(err);
    }
    for ack in final_flushes {
        let _ = ack.send(Ok(()));
    }
    Ok(())
}

fn flush_staging(
    file: &File,
    staging: &mut BTreeMap<u64, Bytes>,
    staging_bytes: &mut usize,
    scratch: &mut Vec<u8>,
    permits: &mut Vec<OwnedSemaphorePermit>,
    coalesced_ops: &AtomicU64,
) -> io::Result<()> {
    if staging.is_empty() {
        *staging_bytes = 0;
        permits.clear();
        return Ok(());
    }

    let entries = std::mem::take(staging);
    *staging_bytes = 0;

    let mut run_start: u64 = 0;
    let mut run_end: u64 = 0;
    let mut single_chunk: Option<Bytes> = None;
    let mut using_scratch = false;

    let result = (|| -> io::Result<()> {
        for (offset, data) in entries {
            if data.is_empty() {
                continue;
            }
            let chunk_end = offset.saturating_add(data.len() as u64);

            if single_chunk.is_none() && !using_scratch {
                run_start = offset;
                run_end = chunk_end;
                single_chunk = Some(data);
                continue;
            }

            if offset <= run_end {
                // Contiguous or overlapping: coalesce into scratch buffer
                if !using_scratch {
                    scratch.clear();
                    if let Some(first) = single_chunk.take() {
                        scratch.extend_from_slice(&first);
                    }
                    using_scratch = true;
                }
                let rel_start = (offset - run_start) as usize;
                let rel_end = rel_start.saturating_add(data.len());
                if rel_end > scratch.len() {
                    scratch.resize(rel_end, 0);
                }
                scratch[rel_start..rel_end].copy_from_slice(&data);
                run_end = run_end.max(chunk_end);
            } else {
                // Disjoint gap: flush current run first
                if using_scratch {
                    write_all_at_offset(file, run_start, scratch)?;
                    scratch.clear();
                    using_scratch = false;
                } else if let Some(first) = single_chunk.take() {
                    write_all_at_offset(file, run_start, &first)?;
                }
                coalesced_ops.fetch_add(1, Ordering::Relaxed);

                run_start = offset;
                run_end = chunk_end;
                single_chunk = Some(data);
            }
        }

        if using_scratch {
            write_all_at_offset(file, run_start, scratch)?;
            scratch.clear();
            coalesced_ops.fetch_add(1, Ordering::Relaxed);
        } else if let Some(first) = single_chunk.take() {
            write_all_at_offset(file, run_start, &first)?;
            coalesced_ops.fetch_add(1, Ordering::Relaxed);
        }

        Ok(())
    })();

    // Release backpressure permits regardless of write outcome so blocked writers unblock.
    permits.clear();
    result
}

fn write_all_at_offset(file: &File, offset: u64, data: &[u8]) -> io::Result<()> {
    if data.is_empty() {
        return Ok(());
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        let mut written = 0usize;
        while written < data.len() {
            let n = file.seek_write(&data[written..], offset.saturating_add(written as u64))?;
            if n == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "failed to write whole buffer in seek_write",
                ));
            }
            written += n;
        }
        Ok(())
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        file.write_all_at(data, offset)
    }

    #[cfg(not(any(windows, unix)))]
    {
        use std::io::{Seek, SeekFrom, Write};
        let mut f = file;
        f.seek(SeekFrom::Start(offset))?;
        f.write_all(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_coalescing_out_of_order_and_finish() {
        let dir = tempfile::tempdir().unwrap();
        let writer =
            PipelinedWriterHandle::with_byte_budget(dir.path(), "coalesce.bin", Some(16), 8, 1024)
                .unwrap();

        let tx = writer.writer();
        tx.write_bytes_at(8, Bytes::from_static(b"WORLD!!!"))
            .await
            .unwrap();
        tx.write_at(0, b"HELLO___".to_vec()).await.unwrap();
        writer.flush().await.unwrap();

        let final_path = writer.finish(None).unwrap();
        let content = std::fs::read(&final_path).unwrap();
        assert_eq!(&content, b"HELLO___WORLD!!!");
    }

    #[tokio::test]
    async fn test_abort_leaves_no_final_file_and_rejects_writes() {
        let dir = tempfile::tempdir().unwrap();
        let mut writer =
            PipelinedWriterHandle::new(dir.path(), "aborted.bin", Some(64), 8).unwrap();
        let tx = writer.writer();
        tx.write_bytes_at(0, Bytes::from_static(b"partial"))
            .await
            .unwrap();

        writer.abort().unwrap();
        assert!(!dir.path().join("aborted.bin").exists());
        assert!(tx
            .write_bytes_at(7, Bytes::from_static(b"more"))
            .await
            .is_err());
    }
}
