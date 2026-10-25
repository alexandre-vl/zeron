use super::*;
use gpui::{
    AnyWindowHandle, Entity, MouseButton, Pixels, PlatformInput, Point, WindowHandle, point,
};
use zeron_ui::browser::BrowserSurface;

/// The first software-rendered frame can take longer than the fixture's
/// initial pause on a loaded runner. Yield to GPUI until X11 maps the window;
/// blocking this thread would prevent the frame we're waiting for.
pub(super) async fn wait_for_visible_window(cx: &mut AsyncApp) -> anyhow::Result<()> {
    // Wayland captures the already-mapped nested compositor's X11 window or
    // its only output.
    if std::env::var("ZERON_BROWSER_CAPTURE_WINDOW").is_ok_and(|id| !id.is_empty())
        || std::env::var_os("ZERON_BROWSER_CAPTURE_COMMAND").is_some()
    {
        return Ok(());
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let windows = std::process::Command::new("xdotool")
            .args([
                "search",
                "--onlyvisible",
                "--pid",
                &std::process::id().to_string(),
            ])
            .output()?;
        if windows.status.success() && !windows.stdout.is_empty() {
            return Ok(());
        }
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "fixture window did not become visible within 10 seconds"
        );
        pause(cx, 50).await;
    }
}

async fn eval(
    page: &Entity<BrowserSurface>,
    script: &str,
    cx: &mut AsyncApp,
) -> anyhow::Result<serde_json::Value> {
    page.read_with(cx, |b, _| b.fixture_eval(script));
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(value) = page.read_with(cx, |b, _| b.fixture_linux_evaluation()) {
            return Ok(value);
        }
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "JavaScript evaluation timed out: {script}"
        );
        pause(cx, 20).await;
    }
}
pub(super) fn dispatch(
    window: WindowHandle<shell::Shell>,
    event: PlatformInput,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    if std::env::var_os("ZERON_BROWSER_NATIVE_POINTER").is_some() {
        let pointer = match &event {
            PlatformInput::MouseDown(e) => Some((e.position, Some(("mousedown", e.button)))),
            PlatformInput::MouseUp(e) => Some((e.position, Some(("mouseup", e.button)))),
            PlatformInput::MouseMove(e) => Some((e.position, None)),
            _ => None,
        };
        if let Some((position, action)) = pointer {
            let id = if let Ok(id) = std::env::var("ZERON_BROWSER_CAPTURE_WINDOW") {
                id
            } else {
                let ids = std::process::Command::new("xdotool")
                    .args([
                        "search",
                        "--onlyvisible",
                        "--pid",
                        &std::process::id().to_string(),
                    ])
                    .output()?;
                String::from_utf8(ids.stdout)?
                    .lines()
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("fixture window not found"))?
                    .to_owned()
            };
            let scale = AnyWindowHandle::from(window).update(cx, |_, w, _| w.scale_factor())?;
            let mut cmd = std::process::Command::new("xdotool");
            cmd.args([
                "mousemove",
                "--window",
                &id,
                &((f32::from(position.x) * scale) as i32).to_string(),
                &((f32::from(position.y) * scale) as i32).to_string(),
            ]);
            if let Some((kind, button)) = action {
                let button = match button {
                    MouseButton::Left => "1",
                    MouseButton::Middle => "2",
                    MouseButton::Right => "3",
                    _ => "1",
                };
                cmd.args([kind, button]);
            }
            anyhow::ensure!(cmd.status()?.success(), "native pointer injection failed");
            return Ok(());
        }
    }
    AnyWindowHandle::from(window).update(cx, |_, w, cx| {
        w.dispatch_event(event, cx);
    })?;
    Ok(())
}
fn click(
    window: WindowHandle<shell::Shell>,
    position: Point<Pixels>,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    dispatch(
        window,
        PlatformInput::MouseDown(gpui::MouseDownEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }),
        cx,
    )?;
    dispatch(
        window,
        PlatformInput::MouseUp(gpui::MouseUpEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }),
        cx,
    )
}
/// A page that repaints its whole viewport every frame: a canvas of moving
/// shapes under a CSS-animated card. `window.benchFrames` counts its frames.
const BENCH_PAGE: &str = "document.body.innerHTML='';document.body.style.margin='0';\
let c=document.createElement('canvas');c.style='position:fixed;inset:0;width:100vw;height:100vh';\
let card=document.createElement('div');card.textContent='Zeron';\
card.style='position:fixed;left:40%;top:40%;padding:24px 48px;font:600 32px system-ui;color:#fff;background:#29483bcc;border-radius:16px;box-shadow:0 12px 40px #0008;animation:spin 2s linear infinite';\
let style=document.createElement('style');style.textContent='@keyframes spin{to{transform:rotate(360deg)}}';\
document.head.append(style);document.body.append(c,card);let g=c.getContext('2d');window.benchFrames=0;\
function frame(t){let w=innerWidth*devicePixelRatio,h=innerHeight*devicePixelRatio;\
if(c.width!==w||c.height!==h){c.width=w;c.height=h}g.fillStyle='#10201a';g.fillRect(0,0,w,h);\
for(let i=0;i<1500;i++){let a=t/900+i*0.37;g.fillStyle='hsl('+(i*7+t/20)%360+' 70% 60%)';\
g.fillRect((Math.sin(a)*0.45+0.5)*w,(Math.cos(a*1.3)*0.45+0.5)*h,14,14)}\
window.benchFrames++;requestAnimationFrame(frame)}requestAnimationFrame(frame);true";

/// CPU seconds this process and its descendants (the browser engine's
/// processes) have used so far.
fn cpu_seconds() -> (f64, f64) {
    let me = std::process::id();
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as f64;
    let mut parents = std::collections::HashMap::new();
    let mut used = std::collections::HashMap::new();
    for entry in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        // Fields after the parenthesized command name: state, ppid, ... utime
        // is the 14th field overall, stime the 15th.
        let fields: Vec<&str> = stat[stat.rfind(')').unwrap_or(0) + 2..].split(' ').collect();
        let field = |i: usize| fields.get(i - 3).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.);
        parents.insert(pid, field(4) as u32);
        used.insert(pid, (field(14) + field(15)) / ticks);
    }
    let descends = |mut pid: u32| {
        while let Some(&parent) = parents.get(&pid) {
            if parent == me {
                return true;
            }
            if parent <= 1 {
                return false;
            }
            pid = parent;
        }
        false
    };
    let engine = used.iter().filter(|(pid, _)| descends(**pid)).map(|(_, s)| s).sum();
    (used.get(&me).copied().unwrap_or(0.), engine)
}

/// Counts the window's display frames until the returned counter is dropped.
fn count_display_frames(window: &gpui::Window) -> std::rc::Rc<std::cell::Cell<u32>> {
    fn next(window: &gpui::Window, count: std::rc::Rc<std::cell::Cell<u32>>) {
        window.on_next_display_frame(move |window, _| {
            if std::rc::Rc::strong_count(&count) > 1 {
                count.set(count.get() + 1);
                next(window, count);
            }
        });
    }
    let count = std::rc::Rc::default();
    next(window, std::rc::Rc::clone(&count));
    count
}

/// Frames per second over `millis`: the page's, counted by the script
/// `frames`, and the display's, from the window's frame callbacks.
async fn frame_rates(
    window: WindowHandle<shell::Shell>,
    page: &Entity<BrowserSurface>,
    frames: &str,
    millis: u64,
    cx: &mut AsyncApp,
) -> anyhow::Result<(f64, f64)> {
    let display = AnyWindowHandle::from(window).update(cx, |_, w, _| count_display_frames(w))?;
    let before = eval(page, frames, cx).await?.as_f64().unwrap_or(0.);
    let (start, shown) = (std::time::Instant::now(), display.get());
    pause(cx, millis).await;
    let after = eval(page, frames, cx).await?.as_f64().unwrap_or(0.);
    let elapsed = start.elapsed().as_secs_f64();
    Ok(((after - before) / elapsed, f64::from(display.get() - shown) / elapsed))
}

/// `ZERON_BROWSER_BENCH=<seconds>`: how fast a page that repaints all of a
/// large viewport every frame renders, and what that costs, with this run's
/// engine and frame path. Writes `bench.json`.
pub async fn bench(
    window: WindowHandle<shell::Shell>,
    page: Entity<BrowserSurface>,
    output: &std::path::Path,
    seconds: f64,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    window.update(cx, |s, _, cx| s.fixture_resize_browser(1400., cx))?;
    eval(&page, BENCH_PAGE, cx).await?;
    pause(cx, 2000).await;
    let (zeron, engine) = cpu_seconds();
    let start = std::time::Instant::now();
    let (fps, display_fps) =
        frame_rates(window, &page, "window.benchFrames", (seconds * 1000.) as u64, cx).await?;
    let (zeron_after, engine_after) = cpu_seconds();
    let elapsed = start.elapsed().as_secs_f64();
    let scale = AnyWindowHandle::from(window).update(cx, |_, w, _| w.scale_factor())?;
    let bounds = page.read_with(cx, |b, _| b.fixture_linux_bounds());
    let result = serde_json::json!({
        "engine": std::env::var("ZERON_BROWSER_ENGINE").unwrap_or_default(),
        "frames": page.read_with(cx, |b, _| b.fixture_linux_frames()),
        "viewport": [f32::from(bounds.size.width) * scale, f32::from(bounds.size.height) * scale],
        "fps": fps,
        "displayFps": display_fps,
        "zeronCpu": (zeron_after - zeron) / elapsed * 100.,
        "engineCpu": (engine_after - engine) / elapsed * 100.,
    });
    println!("{result}");
    std::fs::write(output.join("bench.json"), serde_json::to_vec_pretty(&result)?)?;
    Ok(())
}

pub async fn exercise(
    window: WindowHandle<shell::Shell>,
    page: Entity<BrowserSurface>,
    output: &std::path::Path,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    eval(&page,"document.body.style.paddingTop='0'; document.body.insertAdjacentHTML('afterbegin', `<div id='browser-blur-grid' style='height:140px;background:repeating-conic-gradient(#172f25 0% 25%,#f5f0df 0% 50%) 0 0/16px 16px'></div><input id='browser-input' style='height:32px;width:200px' placeholder='Browser input'><button id='browser-button' onclick='window.browserClicks=(window.browserClicks||0)+1'>Click</button>`); window.browserClicks=0; document.body.addEventListener('pointerdown',()=>window.pagePresses=(window.pagePresses||0)+1); let live=document.createElement('div');live.style='position:fixed;bottom:12px;right:12px;background:#29483b;color:white;padding:8px;border-radius:6px;font:12px monospace';document.body.append(live);window.browserFrames=0;function frame(){live.textContent='LIVE '+(++window.browserFrames);requestAnimationFrame(frame)}frame();true",cx).await?;
    pause(cx, 400).await;
    let bounds = page.read_with(cx, |b, _| b.fixture_linux_bounds());
    let input=eval(&page,"(()=>{let r=document.getElementById('browser-input').getBoundingClientRect();return [r.x+20,r.y+15]})()",cx).await?;
    click(
        window,
        bounds.origin
            + point(
                px(input[0].as_f64().unwrap() as f32),
                px(input[1].as_f64().unwrap() as f32),
            ),
        cx,
    )?;
    // The native click reaches WebKit through the X server after xdotool
    // returns. Wait for the input to take focus instead of racing it with a
    // fixed pause: a loaded runner lost the first keystroke to that race.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while eval(&page, "document.activeElement?.id === 'browser-input'", cx).await? != true {
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "click did not focus the browser input"
        );
        pause(cx, 20).await;
    }
    for c in "hello linux".chars() {
        let key = if c == ' ' {
            "space".into()
        } else {
            c.to_string()
        };
        let stroke = gpui::Keystroke {
            key,
            key_char: Some(c.to_string()),
            modifiers: Default::default(),
        };
        dispatch(
            window,
            PlatformInput::KeyDown(gpui::KeyDownEvent {
                keystroke: stroke.clone(),
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        )?;
        dispatch(
            window,
            PlatformInput::KeyUp(gpui::KeyUpEvent { keystroke: stroke }),
            cx,
        )?;
        pause(cx, 35).await;
    }
    let typed = eval(&page, "document.getElementById('browser-input').value", cx).await?;
    anyhow::ensure!(
        typed == "hello linux",
        "GPUI keyboard input did not reach WebKit: {typed}"
    );
    if let Ok(id) = std::env::var("ZERON_BROWSER_CAPTURE_WINDOW") {
        let scale = AnyWindowHandle::from(window).update(cx, |_, w, _| w.scale_factor())?;
        let x = ((f32::from(bounds.origin.x) + input[0].as_f64().unwrap() as f32) * scale) as i32;
        let y = ((f32::from(bounds.origin.y) + input[1].as_f64().unwrap() as f32) * scale) as i32;
        std::process::Command::new("xdotool")
            .args([
                "mousemove",
                "--window",
                &id,
                &x.to_string(),
                &y.to_string(),
                "click",
                "1",
            ])
            .status()?;
        pause(cx, 200).await;
        // Restore caret to end after the real seat-focus click.
        eval(&page,"(()=>{let e=document.getElementById('browser-input');e.setSelectionRange(e.value.length,e.value.length);return true})()",cx).await?;
    }
    // Compositors give an unfocused window no clipboard: local runs on a hidden
    // workspace (`ZERON_BROWSER_FIXTURE_UNFOCUSED`) type that text instead.
    let clipboard = std::env::var_os("ZERON_BROWSER_FIXTURE_UNFOCUSED").is_none();
    if clipboard {
        cx.update(|cx| cx.write_to_clipboard(gpui::ClipboardItem::new_string(" — café 日本語".into())));
        pause(cx, 200).await;
        let paste = gpui::Keystroke::parse("ctrl-v")?;
        dispatch(
            window,
            PlatformInput::KeyDown(gpui::KeyDownEvent {
                keystroke: paste,
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        )?;
        pause(cx, 150).await;
        anyhow::ensure!(
            eval(&page, "document.getElementById('browser-input').value", cx).await?
                == "hello linux — café 日本語",
            "Unicode clipboard paste failed: {:?}",
            eval(&page, "document.getElementById('browser-input').value", cx).await?
        );
        eval(
            &page,
            "document.getElementById('browser-input').select();true",
            cx,
        )
        .await?;
        dispatch(
            window,
            PlatformInput::KeyDown(gpui::KeyDownEvent {
                keystroke: gpui::Keystroke::parse("ctrl-c")?,
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        )?;
        pause(cx, 150).await;
        anyhow::ensure!(
            cx.update(|cx| cx.read_from_clipboard().and_then(|v| v.text()))
                .as_deref()
                == Some("hello linux — café 日本語"),
            "browser copy did not reach app clipboard"
        );
    } else {
        eval(&page, "(()=>{let e=document.getElementById('browser-input');e.value+=' — café 日本語';return true})()", cx).await?;
    }
    eval(&page,"(()=>{let e=document.getElementById('browser-input');e.setSelectionRange(e.value.length,e.value.length);return true})()",cx).await?;
    window.update(cx, |_, w, cx| {
        page.update(cx, |b, cx| {
            gpui::EntityInputHandler::replace_and_mark_text_in_range(b, None, "にほ", None, w, cx)
        })
    })?;
    pause(cx, 100).await;
    window.update(cx, |_, w, cx| {
        page.update(cx, |b, cx| {
            gpui::EntityInputHandler::replace_and_mark_text_in_range(b, None, "日本語", None, w, cx)
        })
    })?;
    pause(cx, 100).await;
    capture_frame(window, output, "linux-ime-dark", cx).await?;
    window.update(cx, |_, w, cx| {
        page.update(cx, |b, cx| {
            gpui::EntityInputHandler::replace_text_in_range(b, None, "日本語", w, cx)
        })
    })?;
    pause(cx, 150).await;
    anyhow::ensure!(
        eval(&page, "document.getElementById('browser-input').value", cx).await?
            == "hello linux — café 日本語日本語",
        "IME composition did not commit correctly"
    );
    eval(&page,"document.getElementById('browser-button').insertAdjacentHTML('afterend','<select id=browser-select><option value=a>Alpha</option><option value=b>Beta</option></select>');true",cx).await?;
    let select=eval(&page,"(()=>{let r=document.getElementById('browser-select').getBoundingClientRect();return [r.x+10,r.y+10]})()",cx).await?;
    click(
        window,
        bounds.origin
            + point(
                px(select[0].as_f64().unwrap() as f32),
                px(select[1].as_f64().unwrap() as f32),
            ),
        cx,
    )?;
    pause(cx, 250).await;
    anyhow::ensure!(
        page.read_with(cx, |b, _| b.fixture_linux_menu_open()),
        "HTML select did not open a GPUI menu"
    );
    capture_frame(window, output, "linux-select-dark", cx).await?;
    for key in ["down", "enter"] {
        dispatch(
            window,
            PlatformInput::KeyDown(gpui::KeyDownEvent {
                keystroke: gpui::Keystroke::parse(key)?,
                is_held: false,
                prefer_character_input: false,
            }),
            cx,
        )?;
        pause(cx, 100).await;
    }
    anyhow::ensure!(
        eval(&page, "document.getElementById('browser-select').value", cx).await? == "b",
        "select keyboard navigation failed"
    );
    let context = bounds.origin + point(px(300.), px(330.));
    dispatch(
        window,
        PlatformInput::MouseDown(gpui::MouseDownEvent {
            position: context,
            button: MouseButton::Right,
            click_count: 1,
            ..Default::default()
        }),
        cx,
    )?;
    dispatch(
        window,
        PlatformInput::MouseUp(gpui::MouseUpEvent {
            position: context,
            button: MouseButton::Right,
            click_count: 1,
            ..Default::default()
        }),
        cx,
    )?;
    pause(cx, 250).await;
    anyhow::ensure!(
        page.read_with(cx, |b, _| b.fixture_linux_menu_open()),
        "context menu did not open in GPUI"
    );
    capture_frame(window, output, "linux-context-dark", cx).await?;
    dispatch(
        window,
        PlatformInput::KeyDown(gpui::KeyDownEvent {
            keystroke: gpui::Keystroke::parse("escape")?,
            is_held: false,
            prefer_character_input: false,
        }),
        cx,
    )?;
    pause(cx, 150).await;
    anyhow::ensure!(
        !page.read_with(cx, |b, _| b.fixture_linux_menu_open()),
        "context menu did not dismiss"
    );
    for _ in 0..5 {
        for pos in [
            point(bounds.origin.x + px(25.), px(20.)),
            point(bounds.origin.x + px(75.), px(56.)),
            point(bounds.origin.x + px(180.), px(20.)),
        ] {
            dispatch(
                window,
                PlatformInput::MouseMove(gpui::MouseMoveEvent {
                    position: pos,
                    ..Default::default()
                }),
                cx,
            )?;
            pause(cx, 120).await;
            anyhow::ensure!(
                page.read_with(cx, |b, _| b.fixture_native_visible()),
                "hover hid the browser"
            );
        }
    }
    dispatch(
        window,
        PlatformInput::MouseMove(gpui::MouseMoveEvent {
            position: point(bounds.origin.x + px(75.), px(56.)),
            ..Default::default()
        }),
        cx,
    )?;
    pause(cx, 800).await;
    capture_frame(window, output, "linux-tooltip-dark", cx).await?;
    let button=eval(&page,"(()=>{let r=document.getElementById('browser-button').getBoundingClientRect();return [r.x+15,r.y+10]})()",cx).await?;
    click(
        window,
        bounds.origin
            + point(
                px(button[0].as_f64().unwrap() as f32),
                px(button[1].as_f64().unwrap() as f32),
            ),
        cx,
    )?;
    pause(cx, 100).await;
    anyhow::ensure!(
        eval(&page, "window.browserClicks", cx).await? == 1,
        "page button did not activate"
    );
    dispatch(
        window,
        PlatformInput::ScrollWheel(gpui::ScrollWheelEvent {
            position: bounds.origin + point(px(300.), px(300.)),
            delta: gpui::ScrollDelta::Pixels(point(px(0.), px(-250.))),
            ..Default::default()
        }),
        cx,
    )?;
    pause(cx, 250).await;
    anyhow::ensure!(
        eval(&page, "scrollY", cx).await?.as_f64().unwrap() > 50.,
        "scroll did not reach page: {}",
        eval(&page, "JSON.stringify({y: scrollY, height: document.scrollingElement.scrollHeight, viewport: innerHeight, frames: window.browserFrames})", cx).await?
    );
    eval(&page, "scrollTo(0,0);true", cx).await?;
    pause(cx, 200).await;
    let start = point(bounds.origin.x - px(2.), bounds.origin.y + px(100.));
    dispatch(
        window,
        PlatformInput::MouseDown(gpui::MouseDownEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }),
        cx,
    )?;
    let mut widths = Vec::new();
    for delta in [
        20., 60., 100., 140., 100., 60., 20., -20., -60., -100., -140., -100., -60., -20., 0.,
    ] {
        dispatch(
            window,
            PlatformInput::MouseMove(gpui::MouseMoveEvent {
                position: start + point(px(delta), px(0.)),
                pressed_button: Some(MouseButton::Left),
                ..Default::default()
            }),
            cx,
        )?;
        pause(cx, 100).await;
        anyhow::ensure!(
            cx.update(|cx| cx.has_active_drag()),
            "resize failed to start"
        );
        widths.push(f32::from(
            page.read_with(cx, |b, _| b.fixture_linux_bounds().size.width),
        ));
        anyhow::ensure!(
            page.read_with(cx, |b, _| b.fixture_native_visible()),
            "resizing hid browser"
        );
    }
    dispatch(
        window,
        PlatformInput::MouseUp(gpui::MouseUpEvent {
            position: start,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }),
        cx,
    )?;
    pause(cx, 400).await;
    anyhow::ensure!(
        widths.iter().copied().fold(f32::MIN, f32::max)
            - widths.iter().copied().fold(f32::MAX, f32::min)
            > 150.,
        "resize did not change viewport"
    );
    let width = eval(&page, "innerWidth", cx).await?.as_f64().unwrap();
    let layout = f32::from(page.read_with(cx, |b, _| b.fixture_linux_bounds().size.width));
    anyhow::ensure!(
        (width - layout as f64).abs() < 2.,
        "CSS viewport {width} differs from GPUI {layout}"
    );
    capture_frame(window, output, "linux-resize-dark", cx).await?;
    for _ in 0..4 {
        window.update(cx, |s, _, cx| s.fixture_toggle_sidebar(false, cx))?;
        pause(cx, 400).await;
        anyhow::ensure!(
            page.read_with(cx, |b, _| b.fixture_native_visible()),
            "left sidebar lost browser"
        );
    }
    for _ in 0..3 {
        window.update(cx, |s, _, cx| s.fixture_toggle_sidebar(true, cx))?;
        pause(cx, 60).await;
        window.update(cx, |s, _, cx| s.fixture_toggle_sidebar(true, cx))?;
        pause(cx, 400).await;
        anyhow::ensure!(
            page.read_with(cx, |b, _| b.fixture_native_visible()),
            "interrupted sidebar lost browser"
        );
    }
    window.update(cx, |s, _, cx| s.fixture_toggle_sidebar(true, cx))?;
    pause(cx, 400).await;
    anyhow::ensure!(
        !page.read_with(cx, |b, _| b.fixture_native_visible()),
        "closed sidebar kept browser active"
    );
    window.update(cx, |s, _, cx| s.fixture_toggle_sidebar(true, cx))?;
    pause(cx, 400).await;
    anyhow::ensure!(
        page.read_with(cx, |b, _| b.fixture_native_visible()),
        "reopened sidebar lost browser"
    );
    // The animating page renders again as soon as it is shown, before anything
    // else wakes it.
    let frames = page.read_with(cx, |b, _| b.fixture_linux_frames());
    if !frames.is_null() {
        let since = frames["vblankMs"].as_f64();
        anyhow::ensure!(
            since.is_some_and(|ms| ms < 100.),
            "a page shown again gets no vblanks (last {since:?} ms ago)"
        );
    }
    cx.update(|cx| appearance::set_surface(zeron_theme::SurfacePreference::Frosted, cx));
    capture_frame(window, output, "browser-blur-baseline-dark", cx).await?;
    window.update(cx, |s, _, cx| s.fixture_browser_menu(true, cx))?;
    pause(cx, 700).await;
    let presses = eval(&page, "window.pagePresses||0", cx).await?;
    let outside =
        page.read_with(cx, |b, _| b.fixture_linux_bounds().origin) + point(px(350.), px(300.));
    click(window, outside, cx)?;
    // Dismissal includes an exit animation and a deferred repaint. Wait for
    // the real popup lifecycle instead of racing its 120ms removal timer.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while window.update(cx, |s, _, _| s.fixture_browser_menu_mounted())? {
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "outside click did not dismiss browser menu"
        );
        pause(cx, 20).await;
    }
    pause(cx, 100).await;
    anyhow::ensure!(
        eval(&page, "window.pagePresses||0", cx).await? == presses,
        "outside menu click leaked into browser"
    );
    click(window, outside, cx)?;
    pause(cx, 150).await;
    anyhow::ensure!(
        eval(&page, "window.pagePresses||0", cx).await?.as_u64()
            == Some(presses.as_u64().unwrap() + 1),
        "browser input did not return after menu dismissal"
    );
    window.update(cx, |s, _, cx| s.fixture_browser_menu(true, cx))?;
    pause(cx, 500).await;
    capture_frame(window, output, "browser-blur-dark", cx).await?;
    cx.update(|cx| appearance::set_mode(appearance::AppearanceMode::Light, cx));
    pause(cx, 500).await;
    capture_frame(window, output, "browser-blur-light", cx).await?;
    eval(
        &page,
        "document.getElementById('browser-blur-grid').style.background='#263d35'",
        cx,
    )
    .await?;
    pause(cx, 300).await;
    capture_frame(window, output, "browser-blur-solid-light", cx).await?;
    cx.update(|cx| appearance::set_surface(zeron_theme::SurfacePreference::Opaque, cx));
    pause(cx, 300).await;
    capture_frame(window, output, "browser-menu-opaque", cx).await?;
    let bounds = page.read_with(cx, |b, _| b.fixture_linux_bounds());
    let viewport =
        AnyWindowHandle::from(window).update(cx, |_, w, _| f32::from(w.viewport_size().width))?;
    super::validate_blur(
        output,
        // Browser and Terminal are the two permanent rows. Keep this in sync
        // with the compact menu so the lower-right sample stays inside it.
        (f32::from(bounds.origin.x) as f64 + 124., 42., 168., 78.),
        viewport,
    )?;
    window.update(cx, |s, _, cx| s.fixture_browser_menu(false, cx))?;
    cx.update(|cx| {
        appearance::set_mode(appearance::AppearanceMode::Dark, cx);
        appearance::set_surface(zeron_theme::SurfacePreference::Frosted, cx);
    });
    pause(cx, 300).await;
    let frames = eval(&page, "window.browserFrames", cx)
        .await?
        .as_u64()
        .unwrap();
    anyhow::ensure!(frames > 60, "page stopped rendering during interaction");
    // An animating page at rest renders once per display frame: WPE at the
    // display's rate, WebKitGTK, the fallback, at up to 60 Hz.
    let (frame_rate, display_rate) =
        frame_rates(window, &page, "window.browserFrames", 2000, cx).await?;
    let path = page.read_with(cx, |b, _| b.fixture_linux_frames());
    let expected = if path.is_null() { display_rate.min(60.) } else { display_rate };
    anyhow::ensure!(
        frame_rate > expected * 0.9,
        "the page renders {frame_rate:.1} frames/s on a display showing {display_rate:.1}"
    );
    // WebKit keeps about four buffers per page; more means its frames outpace
    // the display.
    if let Some(shared) = path["shared"].as_u64() {
        anyhow::ensure!(shared <= 6, "WebKit keeps {shared} buffers for one page");
    }
    std::fs::write(
        output.join("linux-results.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"keyboard":true,"unicodeClipboard":clipboard,"imeComposition":true,"buttonAndScroll":true,"menuClickIsolation":true,"hoverVisibility":true,"resizeWidths":widths,"cssViewport":width,"liveFrames":frames,"frameRate":frame_rate,"displayRate":display_rate,"frames":path,"sidebarTransitions":true}),
        )?,
    )?;
    Ok(())
}
