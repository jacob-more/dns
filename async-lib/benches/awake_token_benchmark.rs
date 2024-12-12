use std::{future::Future, iter::repeat, sync::{atomic::{AtomicBool, Ordering}, Arc}, task::Context, thread::{self, sleep}, time::Duration};

use async_lib::awake_token::AwakeToken;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use tokio::pin;


fn append_no_contention_benchmark(c: &mut Criterion) {
    let awake_token = AwakeToken::new();

    c.bench_function("Append No Contention", |b|
        b.iter_with_large_drop(|| {
            let waker = futures::task::noop_waker();
            let mut context = Context::from_waker(&waker);
            let awake_token = awake_token.clone();
            let awoken_token = awake_token.awoken();
            pin! { awoken_token };
            let _ = black_box(black_box(awoken_token).poll(black_box(&mut context)));
        })
    );
}

fn append_with_heavy_contention_benchmark(c: &mut Criterion) {
    let mut benchmark_group = c.benchmark_group("Append With Heavy Contention");

    for thread_count in 1_usize..64_usize {
        let awake_token = AwakeToken::new();

        // Start a separate thread that just appends and removes from the list
        // until the benchmark is done running.
        let contention_alive = Arc::new(AtomicBool::new(true));
        let mut contention_threads = Vec::with_capacity(thread_count);
        repeat(0).take(thread_count).for_each(|_| {
            let contention_thread = thread::spawn({
                let awake_token = awake_token.clone();
                let contention_alive = contention_alive.clone();
                move || {
                    let waker = futures::task::noop_waker();
                    let mut context = Context::from_waker(&waker);

                    while contention_alive.load(Ordering::Relaxed) {
                        let awake_token = awake_token.clone();
                        let awoken_token = awake_token.awoken();
                        pin! { awoken_token };
                        let _ = black_box(awoken_token.poll(black_box(&mut context)));
                    }
                }
            });
            contention_threads.push(contention_thread);
        });

        // Give the other threads a moment to start running.
        sleep(Duration::from_secs(1));

        benchmark_group.bench_with_input(BenchmarkId::new("Append With Contention", thread_count), &thread_count, |b, _|
            b.iter(|| {
                let waker = futures::task::noop_waker();
                let mut context = Context::from_waker(&waker);
                let awake_token = awake_token.clone();
                let awoken_token = awake_token.awoken();
                pin! { awoken_token };
                let _ = black_box(black_box(awoken_token).poll(black_box(&mut context)));
            })
        );

        contention_alive.store(false, Ordering::Release);
        contention_threads.into_iter().for_each(|contention_thread| {
            let _ = contention_thread.join();
        });
    }

    benchmark_group.finish();
}

fn append_with_heavy_contention_multiadd_benchmark(c: &mut Criterion) {
    let mut benchmark_group = c.benchmark_group("Append With Heavy Contention Multi-Add");

    for thread_count in 1_usize..64_usize {
        let awake_token = AwakeToken::new();

        // Start a separate thread that just appends and removes from the list
        // until the benchmark is done running.
        let contention_alive = Arc::new(AtomicBool::new(true));
        let mut contention_threads = Vec::with_capacity(thread_count);
        repeat(0).take(thread_count).for_each(|_| {
            let contention_thread = thread::spawn({
                let awake_token = awake_token.clone();
                let contention_alive = contention_alive.clone();
                move || {
                    let waker = futures::task::noop_waker();
                    let mut context = Context::from_waker(&waker);

                    while contention_alive.load(Ordering::Relaxed) {
                        let awake_token_for_awoken = awake_token.clone();
                        let awoken_token1 = awake_token_for_awoken.awoken();
                        pin! { awoken_token1 };
                        let _ = black_box(awoken_token1.poll(black_box(&mut context)));

                        let awake_token_for_awoken = awake_token.clone();
                        let awoken_token2 = awake_token_for_awoken.awoken();
                        pin! { awoken_token2 };
                        let _ = black_box(awoken_token2.poll(black_box(&mut context)));

                        let awake_token_for_awoken = awake_token.clone();
                        let awoken_token3 = awake_token_for_awoken.awoken();
                        pin! { awoken_token3 };
                        let _ = black_box(awoken_token3.poll(black_box(&mut context)));
                    }
                }
            });
            contention_threads.push(contention_thread);
        });

        // Give the other threads a moment to start running.
        sleep(Duration::from_secs(1));

        benchmark_group.bench_with_input(BenchmarkId::new("Append With Contention", thread_count), &thread_count, |b, _|
            b.iter(|| {
                let waker = futures::task::noop_waker();
                let mut context = Context::from_waker(&waker);
                let awake_token = awake_token.clone();
                let awoken_token = awake_token.awoken();
                pin! { awoken_token };
                let _ = black_box(black_box(awoken_token).poll(black_box(&mut context)));
            })
        );

        contention_alive.store(false, Ordering::Release);
        contention_threads.into_iter().for_each(|contention_thread| {
            let _ = contention_thread.join();
        });
    }

    benchmark_group.finish();
}

fn append_with_light_contention_benchmark(c: &mut Criterion) {
    let mut benchmark_group = c.benchmark_group("Append With Light Contention");

    for thread_count in (1_usize..128_usize).into_iter().filter(|x| x % 2 == 1) {
        let awake_token = AwakeToken::new();

        // Start a separate thread that just appends and removes from the list
        // until the benchmark is done running.
        let contention_alive = Arc::new(AtomicBool::new(true));
        let mut contention_threads = Vec::with_capacity(thread_count);
        repeat(0).take(thread_count).for_each(|_| {
            let contention_thread = thread::spawn({
                let awake_token = awake_token.clone();
                let contention_alive = contention_alive.clone();
                move || {
                    let waker = futures::task::noop_waker();
                    let mut context = Context::from_waker(&waker);

                    while contention_alive.load(Ordering::Relaxed) {
                        let awake_token = awake_token.clone();
                        let awoken_token = awake_token.awoken();
                        pin! { awoken_token };
                        let _ = black_box(awoken_token.poll(black_box(&mut context)));

                        sleep(Duration::from_micros(10));
                    }
                }
            });
            contention_threads.push(contention_thread);
        });

        // Give the other threads a moment to start running.
        sleep(Duration::from_secs(1));

        benchmark_group.bench_with_input(BenchmarkId::new("Append With Contention", thread_count), &thread_count, |b, _|
            b.iter(|| {
                let waker = futures::task::noop_waker();
                let mut context = Context::from_waker(&waker);
                let awake_token = awake_token.clone();
                let awoken_token = awake_token.awoken();
                pin! { awoken_token };
                let _ = black_box(black_box(awoken_token).poll(black_box(&mut context)));
            })
        );

        contention_alive.store(false, Ordering::Release);
        contention_threads.into_iter().for_each(|contention_thread| {
            let _ = contention_thread.join();
        });
    }

    benchmark_group.finish();
}

fn append_with_light_contention_multiadd_benchmark(c: &mut Criterion) {
    let mut benchmark_group = c.benchmark_group("Append With Light Contention Multi-Add");

    for thread_count in (1_usize..128_usize).into_iter().filter(|x| x % 2 == 1) {
        let awake_token = AwakeToken::new();

        // Start a separate thread that just appends and removes from the list
        // until the benchmark is done running.
        let contention_alive = Arc::new(AtomicBool::new(true));
        let mut contention_threads = Vec::with_capacity(thread_count);
        repeat(0).take(thread_count).for_each(|_| {
            let contention_thread = thread::spawn({
                let awake_token = awake_token.clone();
                let contention_alive = contention_alive.clone();
                move || {
                    let waker = futures::task::noop_waker();
                    let mut context = Context::from_waker(&waker);

                    while contention_alive.load(Ordering::Relaxed) {
                        let awake_token_for_awoken = awake_token.clone();
                        let awoken_token1 = awake_token_for_awoken.awoken();
                        pin! { awoken_token1 };
                        let _ = black_box(awoken_token1.poll(black_box(&mut context)));

                        let awake_token_for_awoken = awake_token.clone();
                        let awoken_token2 = awake_token_for_awoken.awoken();
                        pin! { awoken_token2 };
                        let _ = black_box(awoken_token2.poll(black_box(&mut context)));

                        let awake_token_for_awoken = awake_token.clone();
                        let awoken_token3 = awake_token_for_awoken.awoken();
                        pin! { awoken_token3 };
                        let _ = black_box(awoken_token3.poll(black_box(&mut context)));

                        sleep(Duration::from_micros(10));
                    }
                }
            });
            contention_threads.push(contention_thread);
        });

        // Give the other threads a moment to start running.
        sleep(Duration::from_secs(1));

        benchmark_group.bench_with_input(BenchmarkId::new("Append With Contention", thread_count), &thread_count, |b, _|
            b.iter(|| {
                let waker = futures::task::noop_waker();
                let mut context = Context::from_waker(&waker);
                let awake_token = awake_token.clone();
                let awoken_token = awake_token.awoken();
                pin! { awoken_token };
                let _ = black_box(black_box(awoken_token).poll(black_box(&mut context)));
            })
        );

        contention_alive.store(false, Ordering::Release);
        contention_threads.into_iter().for_each(|contention_thread| {
            let _ = contention_thread.join();
        });
    }

    benchmark_group.finish();
}


criterion_group!(
    benches,
    append_no_contention_benchmark,
    append_with_light_contention_benchmark,
    append_with_heavy_contention_benchmark,
    append_with_light_contention_multiadd_benchmark,
    append_with_heavy_contention_multiadd_benchmark,
);
criterion_main!(benches);
