// Copyright 2022 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use std::error::Error;
use std::future::Future;

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum EventLoop {
    ThreadPool,
    MultiSingleThreaded,
}

#[derive(Debug)]
pub struct Builder {
    kind: EventLoop,
    thread_count: Option<usize>,
}

impl Builder {
    pub fn new(kind: EventLoop) -> Builder {
        Builder {
            kind,
            thread_count: None,
        }
    }

    pub fn new_thread_pool() -> Builder {
        Builder {
            kind: EventLoop::ThreadPool,
            thread_count: None,
        }
    }

    pub fn new_multi_single_threaded() -> Builder {
        Builder {
            kind: EventLoop::MultiSingleThreaded,
            thread_count: None,
        }
    }

    pub fn worker_threads(self, count: Option<usize>) -> Builder {
        Builder {
            thread_count: count,
            ..self
        }
    }

    pub fn build_and_block_on<F, R>(self, mut f: F) -> Result<(), Box<dyn Error>>
    where
        F: FnMut() -> R,
        R: Future<Output = Result<(), Box<dyn Error>>> + Send + 'static,
    {
        match self.kind {
            EventLoop::ThreadPool => {
                let mut rt = tokio::runtime::Builder::new_multi_thread();
                rt.enable_all();
                if let Some(thread_count) = self.thread_count {
                    rt.worker_threads(thread_count);
                }
                rt.build()?.block_on(f())?
            }
            EventLoop::MultiSingleThreaded => {
                let thread_count = self.thread_count.unwrap_or_else(num_cpus::get);

                let mut children = Vec::with_capacity(thread_count);

                for _i in 0..thread_count {
                    let future = f();
                    children.push(std::thread::spawn(move || {
                        tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build()
                            .unwrap()
                            .block_on(future)
                            .unwrap()
                    }));
                }

                for child in children {
                    let _ = child.join();
                }
            }
        };

        Ok(())
    }
}
