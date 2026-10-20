//! 后台任务只保留最新一份；基础分、生成与语境分一次性交付。

mod job;
mod scored;

use std::sync::mpsc::{SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crate::sentence::SentenceScorer;

use super::{GENERATE_BEAM, GENERATE_MAX_CHARS};
use job::Job;
pub(crate) use scored::Scored;

pub(crate) struct RescoreWorker {
    wake: SyncSender<()>,

    jobs: Arc<Mutex<Option<Job>>>,

    results: Arc<Mutex<Option<Scored>>>,

    handle: Option<JoinHandle<()>>,

    pub context_available: bool,
}

impl RescoreWorker {
    pub fn spawn(scorer: Box<dyn SentenceScorer>) -> Self {
        let context_available = scorer.supports_context();
        let (wake, notifications) = sync_channel(1);
        let jobs: Arc<Mutex<Option<Job>>> = Arc::default();
        let results: Arc<Mutex<Option<Scored>>> = Arc::default();
        let incoming = Arc::clone(&jobs);
        let outgoing = Arc::clone(&results);
        let handle = std::thread::Builder::new()
            .name("qingjian-rescore".to_owned())
            .spawn(move || {
                while notifications.recv().is_ok() {
                    let job = incoming.lock().unwrap_or_else(|e| e.into_inner()).take();
                    let Some(job) = job else { continue };
                    let texts: Vec<&str> = job.texts.iter().map(String::as_str).collect();
                    let scores = scorer.score(&job.context, &job.keys, &texts);
                    let generated = job.generate.map(|keys| {
                        let texts = scorer.generate(&keys, GENERATE_BEAM, GENERATE_MAX_CHARS);
                        (keys, texts)
                    });
                    let semantic: Vec<&str> = job.semantic.iter().map(String::as_str).collect();
                    let gains = if semantic.is_empty() {
                        Vec::new()
                    } else {
                        scorer.context_gain(&job.context, &semantic)
                    };
                    *outgoing.lock().unwrap_or_else(|e| e.into_inner()) = Some(Scored {
                        epoch: job.epoch,
                        context: job.context,
                        keys: job.keys,
                        texts: job.texts,
                        scores,
                        generated,
                        semantic: job.semantic,
                        gains,
                    });
                }
            })
            .ok();
        Self {
            wake,
            jobs,
            results,
            handle,
            context_available,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.handle
            .as_ref()
            .is_some_and(|handle| !handle.is_finished())
    }

    pub fn submit(
        &self,
        epoch: u64,
        context: String,
        keys: String,
        texts: Vec<String>,
        generate: Option<String>,
        semantic: Vec<String>,
    ) {
        *self.jobs.lock().unwrap_or_else(|e| e.into_inner()) = Some(Job {
            epoch,
            context,
            keys,
            texts,
            generate,
            semantic,
        });
        let _ = self.wake.try_send(());
    }

    pub fn poll(&self) -> Option<Scored> {
        self.results
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }

    /// 尚未执行的任务与已完成结果在隐私边界立刻释放；在飞任务靠代次拒收。
    pub fn cancel_pending(&self) {
        self.jobs.lock().unwrap_or_else(|e| e.into_inner()).take();
        self.results
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
    }
}
