//! Native visual evidence: scripted layouts, pages and clicks that save what the real
//! renderer drew. Built only with the `visual-test` feature.
use super::*;
use crate::terminal::screen;
use gpui::WindowHandle;

/// Runs the evidence script the environment asks for against the opened workbench.
pub fn start(
    handle: Option<WindowHandle<workbench::Workbench>>,
    evidence: Option<PathBuf>,
    cx: &mut App,
) {
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_SPLIT_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            for name in ["single","nested","menu","eight"] {
                handle.update(cx,|view,window,cx|{
                    if window.viewport_size().height < px(theme::Theme::WINDOW_HEIGHT) {window.toggle_simple_fullscreen();}
                    window.resize(size(px(theme::Theme::WINDOW_WIDTH),px(theme::Theme::WINDOW_HEIGHT)));window.bounds_changed(cx);
                    view.visual_layout(name,window,cx);
                }).unwrap();
                cx.background_executor().timer(Duration::from_secs(2)).await;
                let root=if name=="menu"{Some(handle.update(cx,|v,_,_|v.visual_bounds()).unwrap())}else{None};
                cx.update_window(handle.into(),|_,window,app|{
                    window.refresh();window.draw(app).clear(app);
                    if let Some(bounds)=root {native_click(window,app,bounds.origin+gpui::point(px(40.),px(80.)),gpui::MouseButton::Right);}
                    assert_eq!(window.viewport_size(),size(px(theme::Theme::WINDOW_WIDTH),px(theme::Theme::WINDOW_HEIGHT)));
                    screen::verify_native_geometry(window,app);
                    std::fs::create_dir_all(&directory).unwrap();
                    window.render_to_image().unwrap().save(directory.join(format!("{name}-native.png"))).unwrap();
                }).unwrap();
                if let Some(bounds)=root {
                    // Native event dispatch tests; simulated mouse, not human/CUA acceptance.
                    for expected in [1,4] {
                        let at=handle.update(cx,|v,_,_|v.visual_menu_point(2)).unwrap();
                        cx.update_window(handle.into(),|_,window,app|native_click(window,app,at,gpui::MouseButton::Left)).unwrap();
                        handle.update(cx,|v,_,_|v.visual_check_panes(expected)).unwrap();
                        if expected==1 {cx.update_window(handle.into(),|_,window,app|native_click(window,app,bounds.origin+gpui::point(px(40.),px(80.)),gpui::MouseButton::Right)).unwrap();}
                    }
                    cx.update_window(handle.into(),|_,window,app|{
                        use gpui::InputEvent;
                        let from=gpui::point(bounds.left()+(bounds.size.width-px(theme::Theme::S6))*0.58+px(theme::Theme::S6/2.),bounds.top()+px(30.));
                        let to=gpui::point(bounds.left()+bounds.size.width*0.67,from.y);
                        window.dispatch_event(gpui::MouseDownEvent{button:gpui::MouseButton::Left,position:from,modifiers:Default::default(),click_count:1,first_mouse:false}.to_platform_input(),app);
                        window.dispatch_event(gpui::MouseMoveEvent{position:to,pressed_button:Some(gpui::MouseButton::Left),modifiers:Default::default()}.to_platform_input(),app);
                        window.dispatch_event(gpui::MouseUpEvent{button:gpui::MouseButton::Left,position:to,modifiers:Default::default(),click_count:1}.to_platform_input(),app);
                        window.refresh();window.draw(app).clear(app);
                    }).unwrap();
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    handle.update(cx,|v,_,_|v.visual_check_saved_ratio()).unwrap();
                    eprintln!("native events PASS: right menu, zoom 4->1->4, divider drag ratio persisted");
                }
            }
            if std::env::var_os("KL_NATIVE_ONLY").is_some() {
                let bounds=handle.update(cx,|v,_,_|v.visual_bounds()).unwrap();
                cx.update_window(handle.into(),|_,w,a|native_click(w,a,bounds.origin+gpui::point(px(40.),px(80.)),gpui::MouseButton::Right)).unwrap();
                let at=handle.update(cx,|v,_,_|v.visual_menu_point(0)).unwrap();
                cx.update_window(handle.into(),|_,w,a|native_click(w,a,at,gpui::MouseButton::Left)).unwrap();
                cx.background_executor().timer(Duration::from_secs(1)).await;
                handle.update(cx,|v,w,a|{v.visual_assert_new_shell(9,w,a);v.visual_split_down(w,a);}).unwrap();
                cx.background_executor().timer(Duration::from_secs(1)).await;
                handle.update(cx,|v,w,a|{v.visual_assert_new_shell(10,w,a);v.visual_close_focused(a);}).unwrap();
                cx.background_executor().timer(Duration::from_secs(1)).await;
                handle.update(cx,|v,_,a|{v.visual_check_panes(9);v.visual_check_closed_processes(1);v.visual_close_focused(a);}).unwrap();
                cx.background_executor().timer(Duration::from_secs(1)).await;
                handle.update(cx,|v,_,_|{v.visual_check_panes(8);v.visual_check_closed_processes(2);}).unwrap();
                eprintln!("native actions PASS: right/down launch 8->9->10, shell cwd and focus, close 10->9->8 stops the closed PTYs");
            }
            cx.update(|cx|cx.quit());
        }).detach();
    }
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_TAB_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            use gpui::InputEvent;
            let executor = cx.background_executor().clone();
            let wait = move |s: u64| executor.timer(Duration::from_millis(s));
            wait(2000).await;
            for _ in 0..2 {
                handle.update(cx, |v, _, cx| v.visual_new_tab(cx)).unwrap();
                wait(1500).await;
            }
            std::fs::create_dir_all(&directory).unwrap();
            let shot = |name: &str, window: &mut gpui::Window, app: &mut App| {
                window.refresh();
                window.draw(app).clear(app);
                window
                    .render_to_image()
                    .unwrap()
                    .save(directory.join(format!("{name}.png")))
                    .unwrap();
            };
            let state = |cx: &mut gpui::AsyncApp| {
                handle
                    .update(cx, |v, w, a| v.visual_tab_state(w, a))
                    .unwrap()
            };
            // The first tab sits right after the sidebar, in the strip under the top bar.
            let tab = gpui::point(
                px(theme::Theme::SIDEBAR + 30.),
                px(theme::Theme::TOP + theme::Theme::TABS / 2.),
            );
            let hover = |at: gpui::Point<gpui::Pixels>| {
                gpui::MouseMoveEvent {
                    position: at,
                    pressed_button: None,
                    modifiers: Default::default(),
                }
                .to_platform_input()
            };
            // Hovering shows the tab's card; the menu must not sit under it.
            cx.update_window(handle.into(), |_, w, a| {
                w.dispatch_event(hover(tab), a);
            })
            .unwrap();
            wait(900).await;
            cx.update_window(handle.into(), |_, w, a| shot("0-tab-hover", w, a))
                .unwrap();
            cx.update_window(handle.into(), |_, w, a| {
                native_click(w, a, tab, gpui::MouseButton::Right)
            })
            .unwrap();
            let (menu, renaming) = state(cx);
            assert!(menu && renaming.is_none(), "right click opens the tab menu");
            // Still over the tab with the menu open: the card stays away.
            cx.update_window(handle.into(), |_, w, a| {
                w.dispatch_event(hover(tab + gpui::point(px(4.), px(0.))), a);
            })
            .unwrap();
            wait(900).await;
            cx.update_window(handle.into(), |_, w, a| shot("1-tab-menu", w, a))
                .unwrap();
            let at = handle
                .update(cx, |v, _, _| v.visual_tab_menu_point(0))
                .unwrap();
            cx.update_window(handle.into(), |_, w, a| {
                native_click(w, a, at, gpui::MouseButton::Left)
            })
            .unwrap();
            wait(300).await;
            let (menu, renaming) = state(cx);
            eprintln!("menu rename: menu={menu} field={renaming:?}");
            assert!(
                !menu && renaming.as_ref().is_some_and(|(_, focused)| *focused),
                "重命名 opens a focused field"
            );
            cx.update_window(handle.into(), |_, w, a| shot("2-rename-selected", w, a))
                .unwrap();
            let key = |key: &str| {
                gpui::KeyDownEvent {
                    keystroke: gpui::Keystroke::parse(key).unwrap(),
                    is_held: false,
                    prefer_character_input: false,
                }
                .to_platform_input()
            };
            cx.update_window(handle.into(), |_, w, a| {
                w.dispatch_event(key("backspace"), a);
            })
            .unwrap();
            wait(300).await;
            eprintln!("after backspace: {:?}", state(cx).1);
            cx.update_window(handle.into(), |_, w, a| shot("3-rename-caret", w, a))
                .unwrap();
            cx.update_window(handle.into(), |_, w, a| {
                w.dispatch_event(key("escape"), a);
            })
            .unwrap();
            wait(300).await;
            assert!(state(cx).1.is_none(), "Esc cancels the rename");
            // A real double click: two presses, the second carrying click_count 2.
            cx.update_window(handle.into(), |_, w, a| {
                for count in [1, 2] {
                    w.dispatch_event(
                        gpui::MouseDownEvent {
                            button: gpui::MouseButton::Left,
                            position: tab,
                            modifiers: Default::default(),
                            click_count: count,
                            first_mouse: false,
                        }
                        .to_platform_input(),
                        a,
                    );
                    w.dispatch_event(
                        gpui::MouseUpEvent {
                            button: gpui::MouseButton::Left,
                            position: tab,
                            modifiers: Default::default(),
                            click_count: count,
                        }
                        .to_platform_input(),
                        a,
                    );
                }
            })
            .unwrap();
            wait(300).await;
            let (_, renaming) = state(cx);
            eprintln!("double click: field={renaming:?}");
            assert!(
                renaming.as_ref().is_some_and(|(_, focused)| *focused),
                "double click opens a focused field"
            );
            cx.update_window(handle.into(), |_, w, a| shot("4-double-click", w, a))
                .unwrap();
            cx.update_window(handle.into(), |_, w, a| {
                w.dispatch_event(key("escape"), a);
            })
            .unwrap();
            wait(300).await;
            // A 出错了 conversation whose resume fails: the second click returns to the
            // resume tab instead of opening another.
            let (before, _) = handle.update(cx, |v, _, _| v.visual_tabs()).unwrap();
            handle
                .update(cx, |v, w, a| v.visual_open_failed_session(w, a))
                .unwrap();
            wait(3000).await;
            let (first, resumed) = handle.update(cx, |v, _, _| v.visual_tabs()).unwrap();
            cx.update_window(handle.into(), |_, w, a| shot("5-failed-resume", w, a))
                .unwrap();
            for _ in 0..3 {
                handle
                    .update(cx, |v, w, a| v.visual_open_failed_session(w, a))
                    .unwrap();
                wait(1500).await;
            }
            let (again, current) = handle.update(cx, |v, _, _| v.visual_tabs()).unwrap();
            eprintln!(
                "failed session tabs: before={before} first={first} after 3 more clicks={again}"
            );
            assert_eq!(first, before + 1, "the first click opens one resume tab");
            assert_eq!(
                (again, &current),
                (first, &resumed),
                "later clicks return to it"
            );
            cx.update_window(handle.into(), |_, w, a| shot("6-failed-reclick", w, a))
                .unwrap();
            eprintln!("tab evidence PASS");
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_PAGE_EVIDENCE").is_some()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            handle
                .update(cx, |_, window, _| {
                    if window.viewport_size().height < px(theme::Theme::WINDOW_HEIGHT) {
                        window.toggle_simple_fullscreen();
                    }
                })
                .unwrap();
            cx.background_executor().timer(Duration::from_secs(1)).await;
            std::fs::create_dir_all(&directory).unwrap();
            let pages = std::env::var("KEEPLINE_PAGE_LIST").unwrap_or_else(|_| {
                "Main,Overview,TodoNew,TodoDetail,TodoStart,Cleanup,Menubar".into()
            });
            for name in pages.split(',').map(str::to_owned).collect::<Vec<_>>() {
                let name = name.as_str();
                handle
                    .update(cx, |view, window, cx| {
                        window.resize(size(
                            px(theme::Theme::WINDOW_WIDTH),
                            px(theme::Theme::WINDOW_HEIGHT),
                        ));
                        window.bounds_changed(cx);
                        view.visual_page(name, window, cx);
                    })
                    .unwrap();
                // Starting an Agent and placing the task text takes a few quiet checks.
                let settle = if name == "TodoStart" { 4 } else { 1 };
                cx.background_executor()
                    .timer(Duration::from_secs(settle))
                    .await;
                cx.update_window(handle.into(), |_, window, app| {
                    assert_eq!(
                        window.viewport_size(),
                        size(
                            px(theme::Theme::WINDOW_WIDTH),
                            px(theme::Theme::WINDOW_HEIGHT)
                        )
                    );
                    window.refresh();
                    window.draw(app).clear(app);
                    let path = directory.join(format!("{}-native.png", name.to_lowercase()));
                    if name == "Menubar" {
                        desktop::snapshot(&path);
                    } else {
                        window.render_to_image().unwrap().save(path).unwrap();
                    }
                    eprintln!(
                        "{name}: native viewport {:?}, scale {}",
                        window.viewport_size(),
                        window.scale_factor()
                    );
                })
                .unwrap();
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
    if let (Some(handle), Some(directory)) = (&handle, evidence.clone())
        && std::env::var_os("KEEPLINE_SPLIT_EVIDENCE").is_none()
        && std::env::var_os("KEEPLINE_TAB_EVIDENCE").is_none()
        && std::env::var_os("KEEPLINE_PAGE_EVIDENCE").is_none()
    {
        let handle = *handle;
        cx.spawn(async move |cx| {
            // Test-only single shots: native renderer output, no desktop capture permission.
            cx.background_executor().timer(Duration::from_secs(2)).await;
            handle
                .update(cx, |_, window, _| {
                    if window.viewport_size().height < px(theme::Theme::WINDOW_HEIGHT) {
                        window.toggle_simple_fullscreen();
                    }
                })
                .unwrap();
            cx.background_executor().timer(Duration::from_secs(1)).await;
            for overview in [false, true] {
                handle
                    .update(cx, |view, window, cx| {
                        window.resize(size(
                            px(theme::Theme::WINDOW_WIDTH),
                            px(theme::Theme::WINDOW_HEIGHT),
                        ));
                        window.bounds_changed(cx);
                        view.visual_view(overview, window, cx);
                    })
                    .unwrap();
                cx.background_executor().timer(Duration::from_secs(1)).await;
                handle
                    .update(cx, |_, window, app| {
                        assert_eq!(
                            window.viewport_size(),
                            size(
                                px(theme::Theme::WINDOW_WIDTH),
                                px(theme::Theme::WINDOW_HEIGHT)
                            ),
                            "visual comparison viewport"
                        );
                        eprintln!(
                            "{}: viewport={:?}, scale={}, mono={}",
                            if overview { "Overview" } else { "Main" },
                            window.viewport_size(),
                            window.scale_factor(),
                            theme::Theme::mono(app).family
                        );
                        if !overview {
                            screen::verify_native_geometry(window, app);
                        }
                        std::fs::create_dir_all(&directory).unwrap();
                        window
                            .render_to_image()
                            .unwrap()
                            .save(directory.join(if overview {
                                "overview-native.png"
                            } else {
                                "main-native.png"
                            }))
                            .unwrap();
                    })
                    .unwrap();
                if !overview {
                    let marked = handle
                        .update(cx, |view, window, cx| {
                            view.visual_marked_overlay(window, cx)
                        })
                        .unwrap();
                    // Render explicitly: an idle/occluded display link may not
                    // have presented the notification yet. Do not hold the root
                    // entity borrowed while draw renders it again.
                    cx.update_window(handle.into(), |_, window, app| {
                            window.refresh();
                            window.draw(app).clear(app);
                            window
                                .render_to_image()
                                .unwrap()
                                .save(directory.join("ime-native.png"))
                                .unwrap();
                            if let Some(marked) = marked {
                                let scale = window.scale_factor();
                                eprintln!("IME rendered underlines: {:?}", window.painted_underlines());
                                assert!(
                                    window.painted_underlines().iter().any(|line| {
                                        let x = line.bounds.origin.x.0 / scale;
                                        let y = line.bounds.origin.y.0 / scale;
                                        (x - f32::from(marked.left())).abs() < 1.
                                            && y >= f32::from(marked.top())
                                            && y <= f32::from(marked.bottom())
                                    }),
                                    "marked underline must be painted at the IME range, not the pane bottom"
                                );
                            }
                        })
                        .unwrap();
                }
            }
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
}

fn native_click(
    window: &mut gpui::Window,
    app: &mut App,
    position: gpui::Point<gpui::Pixels>,
    button: gpui::MouseButton,
) {
    use gpui::InputEvent;
    window.dispatch_event(
        gpui::MouseDownEvent {
            button,
            position,
            modifiers: Default::default(),
            click_count: 1,
            first_mouse: false,
        }
        .to_platform_input(),
        app,
    );
    window.dispatch_event(
        gpui::MouseUpEvent {
            button,
            position,
            modifiers: Default::default(),
            click_count: 1,
        }
        .to_platform_input(),
        app,
    );
    window.refresh();
    window.draw(app).clear(app);
}
