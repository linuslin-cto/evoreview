//! 异步任务队列（对齐 Python `handle_queue`）。

use std::future::Future;
use tracing::{error, info};

pub fn spawn_job<F>(name: &'static str, fut: F)
where
    F: Future<Output = Result<(), String>> + Send + 'static,
{
    tokio::spawn(async move {
        info!(job = name, "queue: job started");
        match fut.await {
            Ok(()) => info!(job = name, "queue: job finished"),
            Err(e) => {
                error!(job = name, error = %e, "queue: job failed");
                let msg = format!("服务出现未知错误: {e}");
                crate::im::send_notification(crate::im::Notification::text(msg)).await;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn spawn_job_runs() {
        let flag = Arc::new(Mutex::new(false));
        let flag2 = flag.clone();
        spawn_job("test", async move {
            *flag2.lock().unwrap() = true;
            Ok(())
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(*flag.lock().unwrap());
    }
}
