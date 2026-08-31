use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use dxlib::interface::stream::{poll, poll_callback};
use tokio_stream::StreamExt;

#[tokio::test]
async fn poll_yields_items() {
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();

    let mut stream = poll(Duration::from_millis(10), false, move || {
        let c = c.clone();
        async move {
            let n = c.fetch_add(1, Ordering::SeqCst);
            Ok::<_, &'static str>(n)
        }
    });

    let first = stream.next().await.unwrap().unwrap();
    let second = stream.next().await.unwrap().unwrap();
    let third = stream.next().await.unwrap().unwrap();

    assert_eq!(first, 0);
    assert_eq!(second, 1);
    assert_eq!(third, 2);
}

#[tokio::test]
async fn poll_yields_errors_without_stopping() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();

    let mut stream = poll(Duration::from_millis(10), false, move || {
        let c = c.clone();
        async move {
            let n = c.fetch_add(1, Ordering::SeqCst);
            if n % 2 == 0 {
                Ok(n)
            } else {
                Err("odd")
            }
        }
    });

    let r1 = stream.next().await.unwrap();
    assert_eq!(r1, Ok(0));

    let r2 = stream.next().await.unwrap();
    assert_eq!(r2, Err("odd"));

    let r3 = stream.next().await.unwrap();
    assert_eq!(r3, Ok(2));
}

#[tokio::test]
async fn poll_callback_invoked_on_each_result() {
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let s = seen.clone();

    let mut stream = poll_callback(
        Duration::from_millis(10),
        false,
        || async { Ok::<_, &'static str>(42) },
        move |r: &Result<i32, &'static str>| {
            s.lock().unwrap().push(match r {
                Ok(v) => *v,
                Err(_) => -1,
            });
        },
    );

    stream.next().await.unwrap().unwrap();
    stream.next().await.unwrap().unwrap();
    stream.next().await.unwrap().unwrap();

    assert_eq!(*seen.lock().unwrap(), vec![42, 42, 42]);
}

#[tokio::test]
async fn poll_callback_sees_errors() {
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    let s = seen.clone();
    let toggle = Arc::new(AtomicUsize::new(0));
    let t = toggle.clone();

    let mut stream = poll_callback(
        Duration::from_millis(10),
        false,
        move || {
            let t = t.clone();
            async move {
                if t.fetch_add(1, Ordering::SeqCst) % 2 == 0 {
                    Ok("ok")
                } else {
                    Err("fail")
                }
            }
        },
        move |r: &Result<&str, &str>| {
            let label = match r {
                Ok(v) => v.to_string(),
                Err(e) => format!("err:{e}"),
            };
            s.lock().unwrap().push(label);
        },
    );

    let r1 = stream.next().await.unwrap();
    let r2 = stream.next().await.unwrap();

    assert_eq!(r1, Ok("ok"));
    assert_eq!(r2, Err("fail"));

    assert_eq!(*seen.lock().unwrap(), vec!["ok".to_string(), "err:fail".to_string()]);
}
