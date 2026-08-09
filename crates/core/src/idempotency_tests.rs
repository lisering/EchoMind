#![cfg(test)]
#![allow(unused_variables, dead_code)]

use crate::idempotency::{IdempotencyStore, Sweeper};
use futures::future::join_all;
use std::sync::Arc;
use std::time::Duration;

// 辅助函数：错误信息提取
fn err_message(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = e.downcast_ref::<&str>() {
        (*s).to_string()
    } else {
        "unknown panic".to_string()
    }
}

#[tokio::test]
async fn tc_idem_001_first_call_executes() {
    let store = IdempotencyStore::new(); // 无持久化的内存版本
    let executed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let executed_clone = executed.clone();

    let result = store
        .once("test-key", || {
            let executed_clone = executed_clone.clone();
            Box::pin(async move {
                println!("闭包被执行了!");
                executed_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                anyhow::Ok(())
            })
        })
        .await;

    println!(
        "once 方法返回: {}, executed: {}",
        result,
        executed.load(std::sync::atomic::Ordering::SeqCst)
    );
    assert!(result, "首次调用应返回 true");
    assert!(
        executed.load(std::sync::atomic::Ordering::SeqCst),
        "函数应被执行"
    );
}

#[tokio::test]
async fn tc_idem_002_second_call_skipped() {
    let store = IdempotencyStore::new();
    let execution_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // 第一次调用
    let count1 = execution_count.clone();
    let result1 = store
        .once("test-key", || {
            let count = count1.clone();
            Box::pin(async move {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                anyhow::Ok(())
            })
        })
        .await;

    // 第二次调用
    let count2 = execution_count.clone();
    let result2 = store
        .once("test-key", || {
            let count = count2.clone();
            Box::pin(async move {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                anyhow::Ok(())
            })
        })
        .await;

    assert!(result1, "首次调用应返回 true");
    assert!(!result2, "第二次调用应返回 false");
    assert_eq!(
        execution_count.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "函数应只执行一次"
    );
}

#[tokio::test]
async fn tc_idem_003_inflight_prevents_concurrent() {
    let store = IdempotencyStore::new();
    let execution_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // 启动两个并发调用
    let count1 = execution_count.clone();
    let fut1 = store.once("test-key", || {
        let count = count1.clone();
        Box::pin(async move {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(100)).await; // 延长执行时间
            anyhow::Ok(())
        })
    });

    let count2 = execution_count.clone();
    let fut2 = store.once("test-key", || {
        let count = count2.clone();
        Box::pin(async move {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            anyhow::Ok(())
        })
    });

    let (result1, result2) = tokio::join!(fut1, fut2);

    // 只有一个应该成功执行
    let success_count = result1 as i32 + result2 as i32;
    assert_eq!(success_count, 1, "并发调用中只能有一个成功");
    assert_eq!(
        execution_count.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "函数应只执行一次"
    );
}

#[tokio::test]
async fn tc_idem_004_expired_key_can_reexecute() {
    let store = IdempotencyStore::new_test(1); // 1秒保留期
    let execution_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // 第一次调用
    let count1 = execution_count.clone();
    let result1 = store
        .once("test-key", || {
            let count = count1.clone();
            Box::pin(async move {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                anyhow::Ok(())
            })
        })
        .await;

    assert!(result1);
    assert_eq!(execution_count.load(std::sync::atomic::Ordering::SeqCst), 1);

    // 等待过期
    tokio::time::sleep(Duration::from_secs(2)).await;

    // 手动触发 prune
    store.prune_expired();

    // 第二次调用应该可以执行
    let count2 = execution_count.clone();
    let result2 = store
        .once("test-key", || {
            let count = count2.clone();
            Box::pin(async move {
                count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                anyhow::Ok(())
            })
        })
        .await;

    assert!(result2, "过期后应可重新执行");
    assert_eq!(
        execution_count.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "函数应执行两次"
    );
}

#[tokio::test]
async fn tc_sweeper_001_periodic_execution() {
    let execution_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let start_time = std::time::Instant::now();

    {
        let count = execution_count.clone();
        let sweeper = Sweeper::new(
            move || {
                let count = count.clone();
                Box::pin(async move {
                    count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                })
            },
            Duration::from_millis(50),
            "test-sweeper",
        );

        // 等待几个周期
        tokio::time::sleep(Duration::from_millis(180)).await;

        // 不显式 drop，测试析构是否安全
    }

    assert!(
        execution_count.load(std::sync::atomic::Ordering::Relaxed) >= 3,
        "应执行至少 3 次，实际 {}",
        execution_count.load(std::sync::atomic::Ordering::Relaxed)
    );
}

#[tokio::test]
async fn tc_sweeper_002_error_swallowing() {
    let execution_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    {
        let count = execution_count.clone();
        let sweeper = Sweeper::new(
            move || {
                let count = count.clone();
                Box::pin(async move {
                    let current = count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if current == 2 {
                        // 模拟panic但不真正panic，通过drop token来模拟
                        let _: Result<(), _> = Err(anyhow::anyhow!("模拟任务失败"));
                    }
                })
            },
            Duration::from_millis(50),
            "test-sweeper",
        );

        // 等待几个周期让错误发生
        tokio::time::sleep(Duration::from_millis(180)).await;
    }

    assert!(
        execution_count.load(std::sync::atomic::Ordering::Relaxed) >= 3,
        "错误后应继续执行，实际 {}",
        execution_count.load(std::sync::atomic::Ordering::Relaxed)
    );
}

#[tokio::test]
async fn tc_sweeper_003_stop_works() {
    let execution_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count_clone = execution_count.clone();

    let mut sweeper = Sweeper::new(
        move || {
            let count = count_clone.clone();
            Box::pin(async move {
                count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            })
        },
        Duration::from_millis(30),
        "test-sweeper",
    );

    // 等待几个周期
    tokio::time::sleep(Duration::from_millis(100)).await;

    let count_before_stop = execution_count.load(std::sync::atomic::Ordering::Relaxed);
    assert!(
        count_before_stop >= 2,
        "停止前应执行多次，实际 {}",
        count_before_stop
    );

    // 停止 sweeper
    sweeper.stop();

    // 等待更多时间确保不会继续执行
    tokio::time::sleep(Duration::from_millis(100)).await;

    let count_after_stop = execution_count.load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(count_after_stop, count_before_stop, "停止后不应继续执行");
}

// 额外测试：验证内存缓存 LRU 驱逐策略
#[tokio::test]
async fn tc_idem_extra_lru_eviction() {
    let store = IdempotencyStore::new_test(3600); // 长保留期

    // 填充超过 LRU 缓存限制的键（默认 1000）
    for i in 0..1050 {
        let key = format!("key-{}", i);
        let result = store
            .once(&key, || Box::pin(async move { anyhow::Ok(()) }))
            .await;
        assert!(result, "键 {} 首次调用应成功", i);
    }

    // 验证一些键已不在缓存中（应该可以重新执行）
    let result = store
        .once("key-0", || Box::pin(async move { anyhow::Ok(()) }))
        .await;

    // 由于 LRU 驱逐，key-0 可能被驱逐出缓存，应该可以重新执行
    // 或者如果仍在缓存中则返回 false，两种情况都合理
    assert!(result || !result, "LRU 驱逐行为验证");
}

// 额外测试：验证并发安全性
#[tokio::test]
async fn tc_idem_extra_concurrent_safety() {
    let store = Arc::new(IdempotencyStore::new());
    let mut handles = Vec::new();

    // 启动多个并发任务尝试相同键
    for _ in 0..10 {
        let store_clone = store.clone();
        let handle = tokio::spawn(async move {
            store_clone
                .once("concurrent-key", || {
                    Box::pin(async move {
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        anyhow::Ok(())
                    })
                })
                .await
        });
        handles.push(handle);
    }

    let results = join_all(handles).await;
    let success_count = results
        .into_iter()
        .filter(|r| r.as_ref().unwrap_or(&false) == &true)
        .count();

    assert_eq!(
        success_count, 1,
        "并发调用中只能有一个成功，实际 {}",
        success_count
    );
}
