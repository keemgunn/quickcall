use std::sync::{Arc, Mutex};

use quickcall::progress::{Capture, CreateProgressOptions, create_progress};

#[test]
fn writes_live_warnings_immediately_and_quiet_writes_nothing() {
    let live_stream = Capture::new();
    let live = create_progress(CreateProgressOptions {
        live: true,
        tool: String::new(),
        model: None,
        stream: Some(live_stream.clone()),
        is_tty: false,
        now_ms: None,
    });
    live.warn("qc_thinking is ignored for tool 'cursor' (encode effort in the model slug)");
    assert_eq!(
        live_stream.text(),
        "[WARNING] qc_thinking is ignored for tool 'cursor' (encode effort in the model slug)\n"
    );

    let quiet_stream = Capture::new();
    let quiet = create_progress(CreateProgressOptions {
        live: false,
        tool: String::new(),
        model: None,
        stream: Some(quiet_stream.clone()),
        is_tty: true,
        now_ms: None,
    });
    quiet.start();
    quiet.warn("should not appear");
    quiet.fail("should not appear either");
    quiet.stop();
    assert_eq!(quiet_stream.text(), "");
}

#[test]
fn shows_tool_and_elapsed_on_the_live_wait_line() {
    let stream = Capture::new();
    let current = Arc::new(Mutex::new(0u64));
    let now = {
        let current = Arc::clone(&current);
        Arc::new(move || *current.lock().expect("now")) as Arc<dyn Fn() -> u64 + Send + Sync>
    };
    let progress = create_progress(CreateProgressOptions {
        live: true,
        tool: "pi".into(),
        model: None,
        stream: Some(stream.clone()),
        is_tty: true,
        now_ms: Some(now),
    });
    progress.start();
    assert!(
        stream.text().contains(" ⋅ pi ⋅ 000s"),
        "got {:?}",
        stream.text()
    );
    *current.lock().expect("now") = 12_000;
    progress.tick();
    assert!(
        stream.text().contains(" ⋅ pi ⋅ 012s"),
        "got {:?}",
        stream.text()
    );
    progress.stop();
}

#[test]
fn shows_tool_model_and_elapsed_on_the_live_wait_line() {
    let stream = Capture::new();
    let current = Arc::new(Mutex::new(0u64));
    let now = {
        let current = Arc::clone(&current);
        Arc::new(move || *current.lock().expect("now")) as Arc<dyn Fn() -> u64 + Send + Sync>
    };
    let progress = create_progress(CreateProgressOptions {
        live: true,
        tool: "cursor".into(),
        model: Some("composer-2.5".into()),
        stream: Some(stream.clone()),
        is_tty: true,
        now_ms: Some(now),
    });
    progress.start();
    assert!(
        stream.text().contains(" ⋅ cursor ⋅ composer-2.5 ⋅ 000s"),
        "got {:?}",
        stream.text()
    );
    *current.lock().expect("now") = 12_000;
    progress.tick();
    assert!(
        stream.text().contains(" ⋅ cursor ⋅ composer-2.5 ⋅ 012s"),
        "got {:?}",
        stream.text()
    );
    progress.stop();
}
