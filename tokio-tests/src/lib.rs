#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use signal_hook::iterator::Signals;
    use tokio::runtime::Runtime;
    use tokio::time;

    use futures::future;
    use futures::stream::StreamExt;

    fn send_sig(sig: libc::c_int) {
        unsafe { libc::raise(sig) };
    }

    #[test]
    fn repeated() {
        let mut runtime = Runtime::new().unwrap();

        runtime.block_on(async {
            let signals = Signals::new(&[signal_hook::SIGUSR1])
                .unwrap()
                .into_async()
                .unwrap()
                .take(20)
                .map(|r| r.unwrap())
                .map(|sig| {
                    assert_eq!(sig, signal_hook::SIGUSR1);
                    send_sig(signal_hook::SIGUSR1);
                })
                .collect::<()>();
            send_sig(signal_hook::SIGUSR1);

            signals.await
        });
    }

    /// A test where we actually wait for something ‒ the stream/reactor goes to sleep.
    #[test]
    fn delayed() {
        let mut runtime = Runtime::new().unwrap();

        const CNT: usize = 10;
        let cnt = Arc::new(AtomicUsize::new(0));
        let inc_cnt = Arc::clone(&cnt);

        runtime.block_on(async {
            let signals = Signals::new(&[signal_hook::SIGUSR1, signal_hook::SIGUSR2])
                .unwrap()
                .into_async()
                .unwrap()
                .map(|r| r.unwrap())
                .filter(|sig| future::ready(*sig == signal_hook::SIGUSR2))
                .take(CNT)
                .map(move |_| {
                    inc_cnt.fetch_add(1, Ordering::Relaxed);
                });

            let senders =
                time::interval(Duration::from_millis(250)).map(|_| send_sig(signal_hook::SIGUSR2));

            signals.zip(senders).map(drop).collect::<()>().await
        });

        // Just make sure it didn't terminate prematurely
        assert_eq!(CNT, cnt.load(Ordering::Relaxed));
    }

    /// A test where we actually wait for something ‒ the stream/reactor goes to sleep.
    ///
    /// Similar to the above, we try to create it explicitly inside one runtime and run it later
    /// on. Similar to previous versions that could take a handle.
    #[test]
    fn delayed_exp_runtime() {
        let mut runtime = Runtime::new().unwrap();

        let handle = runtime.handle();

        const CNT: usize = 10;
        let cnt = Arc::new(AtomicUsize::new(0));
        let inc_cnt = Arc::clone(&cnt);

        // The sync version
        let signals = Signals::new(&[signal_hook::SIGUSR1, signal_hook::SIGUSR2]).unwrap();

        // Async version
        let signals = handle.enter(|| signals.into_async().unwrap());

        let signals = signals
            .map(|r| r.unwrap())
            .filter(|sig| future::ready(*sig == signal_hook::SIGUSR2))
            .take(CNT)
            .map(move |_| {
                inc_cnt.fetch_add(1, Ordering::Relaxed);
            });

        runtime.block_on(async {
            let senders =
                time::interval(Duration::from_millis(250)).map(|_| send_sig(signal_hook::SIGUSR2));

            signals.zip(senders).map(drop).collect::<()>().await
        });

        // Just make sure it didn't terminate prematurely
        assert_eq!(CNT, cnt.load(Ordering::Relaxed));
    }
}
