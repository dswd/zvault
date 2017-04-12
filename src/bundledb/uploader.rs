use ::prelude::*;

use std::sync::atomic::{Ordering, AtomicBool, AtomicUsize};
use std::sync::{Mutex, Condvar, Arc};
use std::{mem, fs, thread};
use std::path::{Path, PathBuf};

use crossbeam::sync::MsQueue;


pub struct BundleUploader {
    capacity: usize,
    error_present: AtomicBool,
    error: Mutex<Option<BundleDbError>>,
    waiting: AtomicUsize,
    queue: MsQueue<Option<(PathBuf, PathBuf)>>,
    wait: (Condvar, Mutex<()>)
}

impl BundleUploader {
    pub fn new(capacity: usize) -> Arc<Self> {
        let self_ = Arc::new(BundleUploader {
            capacity: capacity,
            error_present: AtomicBool::new(false),
            error: Mutex::new(None),
            waiting: AtomicUsize::new(0),
            queue: MsQueue::new(),
            wait: (Condvar::new(), Mutex::new(()))
        });
        let self2 = self_.clone();
        thread::Builder::new().name("uploader".to_string()).spawn(move || self2.worker_thread()).unwrap();
        self_
    }

    fn get_status(&self) -> Result<(), BundleDbError> {
        if self.error_present.load(Ordering::SeqCst) {
            let mut error = None;
            mem::swap(&mut error, &mut self.error.lock().unwrap());
            Err(error.unwrap())
        } else {
            Ok(())
        }
    }

    pub fn queue(&self, local_path: PathBuf, remote_path: PathBuf) -> Result<(), BundleDbError> {
        while self.waiting.load(Ordering::SeqCst) >= self.capacity {
            debug!("Upload queue is full, waiting for slots");
            let _ = self.wait.0.wait(self.wait.1.lock().unwrap()).unwrap();
        }
        trace!("Adding to upload queue: {:?}", local_path);
        if !self.error_present.load(Ordering::SeqCst) {
            self.waiting.fetch_add(1, Ordering::SeqCst);
            self.queue.push(Some((local_path, remote_path)));
        }
        self.get_status()
    }

    pub fn finish(&self) -> Result<(), BundleDbError> {
        if !self.error_present.load(Ordering::SeqCst) {
            self.waiting.fetch_add(1, Ordering::SeqCst);
            self.queue.push(None);
        }
        while self.waiting.load(Ordering::SeqCst) > 0 {
            let _ = self.wait.0.wait(self.wait.1.lock().unwrap());
        }
        self.get_status()
    }

    fn worker_thread_inner(&self) -> Result<(), BundleDbError> {
        while let Some((src_path, dst_path)) = self.queue.pop() {
            trace!("Uploading {:?} to {:?}", src_path, dst_path);
            self.waiting.fetch_sub(1, Ordering::SeqCst);
            self.wait.0.notify_all();
            let folder = dst_path.parent().unwrap();
            try!(fs::create_dir_all(&folder).context(&folder as &Path));
            try!(fs::copy(&src_path, &dst_path).context(&dst_path as &Path));
            try!(fs::remove_file(&src_path).context(&src_path as &Path));
            debug!("Uploaded {:?} to {:?}", src_path, dst_path);
        }
        Ok(())
    }

    fn worker_thread(&self) {
        if let Err(err) = self.worker_thread_inner() {
            *self.error.lock().unwrap() = Some(err);
            self.error_present.store(true, Ordering::SeqCst);
        }
        self.waiting.swap(0, Ordering::SeqCst);
        self.wait.0.notify_all();
    }
}
