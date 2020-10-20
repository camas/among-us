use std::{
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    time::Instant,
};

use client::{Client, ClientSettings, EventHandler, MainServer, ScanSettings};
use common::data::GameListing;
use glium::{
    glutin::{
        self, dpi::LogicalSize, event::Event, event::WindowEvent, event_loop::ControlFlow,
        event_loop::EventLoop, window::WindowBuilder,
    },
    Display, Surface,
};
use imgui::*;
use imgui_glium_renderer::Renderer;
use imgui_winit_support::{HiDpiMode, WinitPlatform};

pub fn run() {
    // Game scanning
    let (ask_scan_send, ask_scan_recv) = mpsc::channel();
    let (scan_results_send, scan_results_recv) = mpsc::channel();
    let _scan_thread = std::thread::spawn(move || {
        let settings = ScanSettings {
            connect_username: "scan".to_string(),
            max_requests: 1,
            cache_size: 1,
            ..ScanSettings::default()
        };

        let callback = |listings: Vec<GameListing>| {
            scan_results_send.send(listings).unwrap();
            if ask_scan_recv.recv().is_err() {
                return false;
            }
            true
        };

        Client::server_scan(settings, callback);
    });

    // Main client
    enum JoinGameInfo {
        Listing(GameListing),
        Code(String),
    }
    #[derive(Debug, Clone)]
    enum InfoOut {
        ChatMessage {
            player_name: String,
            message: String,
        },
    }
    let (join_game_send, join_game_recv) = mpsc::channel();
    let (info_out_send, info_out_recv) = mpsc::channel();
    std::thread::spawn(move || {
        // Wait for initial connection request
        let mut game_info = match join_game_recv.recv() {
            Ok(info) => info,
            Err(_) => return,
        };
        loop {
            // Client settings
            let settings = ClientSettings {
                connect_username: "oregano".to_string(),
                game_username: "oregano".to_string(),
                ..ClientSettings::default()
            };

            // Handler
            let (stop_send, stop_recv) = mpsc::channel();
            let handler = ClientHandler {
                stop_recv,
                info_out_send: info_out_send.clone(),
            };

            // Run
            std::thread::spawn(move || match game_info {
                JoinGameInfo::Listing(listing) => Client::run_game(handler, listing, settings),
                JoinGameInfo::Code(code) => {
                    Client::run_game_code(handler, MainServer::Europe, &code, settings)
                }
            });

            // Wait for connection request
            game_info = match join_game_recv.recv() {
                Ok(info) => info,
                Err(_) => {
                    // Send stop request and exit
                    let _ = stop_send.send(());
                    return;
                }
            };

            // Disconnect old thread
            let _ = stop_send.send(());
        }

        struct ClientHandler {
            stop_recv: Receiver<()>,
            info_out_send: Sender<InfoOut>,
        }

        impl EventHandler for ClientHandler {
            fn packet_received(&mut self, client: &mut Client) {
                if self.stop_recv.try_recv().is_ok() {
                    client.disconnect();
                }
            }

            fn chat_message(&mut self, client: &mut Client, player_id: i32, message: String) {
                let player_name = match client.net_objects.get_player_control(player_id) {
                    Some(control) => control.name.as_ref().unwrap().clone(),
                    None => "???".to_string(),
                };
                let _ = self.info_out_send.send(InfoOut::ChatMessage {
                    player_name,
                    message,
                });
            }
        }
    });

    // Initialize imgui
    let mut system = System::init("Among Us Client", 1024., 768.);
    system.imgui.io_mut().config_flags |= imgui::ConfigFlags::DOCKING_ENABLE;

    // Style
    let style = system.imgui.style_mut();
    style.frame_rounding = 0.;
    style.child_rounding = 0.;
    style.tab_rounding = 0.;
    style.window_rounding = 0.;
    style.popup_rounding = 0.;
    style.scrollbar_rounding = 0.;
    style.grab_rounding = 0.;
    style.window_border_size = 0.;

    // State init
    struct State {
        game_code_input: ImString,
        scan_results: Vec<GameListing>,
        messages: Vec<(String, String)>,
    }

    impl Default for State {
        fn default() -> Self {
            Self {
                game_code_input: ImString::with_capacity(6),
                scan_results: Vec::new(),
                messages: Vec::new(),
            }
        }
    }

    // Gui loop
    let start = Instant::now();
    let mut last_width = 0;
    let mut last_height = 0;
    let mut state = State::default();
    system.main_loop(move |_run, ui, width, height| {
        // Read messages from threads
        match scan_results_recv.try_recv() {
            Ok(results) => {
                if !results.is_empty() {
                    state.scan_results = results
                }
            }
            Err(TryRecvError::Empty) => (),
            Err(value) => {
                eprintln!("{}", value);
                return;
            }
        }
        loop {
            match info_out_recv.try_recv() {
                Ok(value) => match value {
                    InfoOut::ChatMessage {
                        player_name,
                        message,
                    } => state.messages.push((player_name, message)),
                },
                Err(TryRecvError::Empty) => break,
                Err(value) => {
                    eprintln!("{}", value);
                    return;
                }
            }
        }

        // Client window
        Window::new(im_str!("Hello world"))
            .resizable(false)
            .movable(false)
            .build(ui, || {
                ui.text(im_str!("Hello world!"));
                ui.separator();
                let mouse_pos = ui.io().mouse_pos;
                ui.text(format!(
                    "Mouse Position: ({:.1},{:.1})",
                    mouse_pos[0], mouse_pos[1]
                ));
                ui.separator();
                ui.text("Progress bar!");
                ProgressBar::new(((start.elapsed().as_millis() % 5000) as f32 / 5000.) as f32)
                    .size([200., 20.])
                    .build(ui);
            });

        // Game listing window
        Window::new(im_str!("Games"))
            .resizable(false)
            .movable(false)
            .build(ui, || {
                if ui.button(im_str!("Scan"), [ui.window_content_region_width(), 20.]) {
                    ask_scan_send.send(true).unwrap();
                }
                for listing in state.scan_results.iter() {
                    ui.text(format!("{} {}", listing.id, listing.host_username));
                    ui.same_line(0.);
                    if ui.small_button(im_str!("Join")) {
                        // Join game
                        if let Err(_error) =
                            join_game_send.send(JoinGameInfo::Listing(listing.to_owned()))
                        {
                            todo!()
                        }
                    }
                    ui.text(format!(
                        "{:>2}/{:<2} {:?}",
                        listing.player_count, listing.max_players, listing.map_id
                    ));
                    ui.separator();
                }
                ui.separator();
                ui.input_text(im_str!("Game code"), &mut state.game_code_input)
                    .chars_noblank(true)
                    .chars_uppercase(true)
                    .build();
                if ui.button(im_str!("Join"), [ui.window_content_region_width(), 20.]) {
                    if let Err(_error) = join_game_send.send(JoinGameInfo::Code(
                        state.game_code_input.to_str().to_string(),
                    )) {
                        todo!()
                    }
                }
            });

        // Chat window
        Window::new(im_str!("Chat"))
            .resizable(false)
            .movable(false)
            .build(ui, || {
                for (name, message) in state.messages.iter() {
                    ui.text(format!("{}: {}", name, message));
                }
            });

        // Dock windows if resized or first run
        if width != last_width || height != last_height {
            last_height = height;
            last_width = width;
            Dock::new().build(|root| {
                root.position([0., 0.])
                    .size([width as f32 / 2., height as f32 / 2.])
                    .split(
                        Direction::Left,
                        0.2,
                        |left| {
                            left.dock_window(im_str!("Games"));
                        },
                        |right| {
                            right.split(
                                Direction::Right,
                                0.25,
                                |right| {
                                    right.dock_window(im_str!("Chat"));
                                },
                                |left| {
                                    left.dock_window(im_str!("Hello world"));
                                },
                            );
                        },
                    );
            });
        }
    })
}

/// Handles window backend stuff
struct System {
    event_loop: EventLoop<()>,
    display: Display,
    imgui: Context,
    platform: WinitPlatform,
    renderer: Renderer,
    _font_size: f32,
}

impl System {
    fn init(title: &str, width: f64, height: f64) -> System {
        let builder = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(LogicalSize::new(width, height));
        let event_loop = EventLoop::new();
        let context = glutin::ContextBuilder::new().with_vsync(true);
        let display =
            Display::new(builder, context, &event_loop).expect("Failed to create display");

        let mut imgui = Context::create();
        imgui.set_ini_filename(None);

        let mut platform = WinitPlatform::init(&mut imgui);
        {
            let gl_window = display.gl_window();
            let window = gl_window.window();
            platform.attach_window(imgui.io_mut(), window, HiDpiMode::Rounded);
        }

        let hidpi_factor = platform.hidpi_factor();
        let font_size = (9. * hidpi_factor) as f32;
        imgui.fonts().add_font(&[
            FontSource::TtfData {
                data: include_bytes!("../../resources/RobotoMono-Regular.ttf"),
                size_pixels: font_size * 1.18,
                config: Some(FontConfig {
                    glyph_offset: [0., -1.4],
                    ..FontConfig::default()
                }),
            },
            FontSource::DefaultFontData {
                config: Some(FontConfig {
                    size_pixels: font_size,
                    ..FontConfig::default()
                }),
            },
        ]);

        imgui.io_mut().font_global_scale = (1. / hidpi_factor) as f32;

        let renderer = Renderer::init(&mut imgui, &display).expect("Failed to create renderer");
        System {
            event_loop,
            display,
            imgui,
            platform,
            renderer,
            _font_size: font_size,
        }
    }

    fn main_loop<F>(self, mut run_ui: F)
    where
        F: FnMut(&mut bool, &mut Ui, u32, u32) + 'static,
    {
        let System {
            event_loop,
            display,
            mut imgui,
            mut platform,
            mut renderer,
            ..
        } = self;
        let mut last_frame = Instant::now();

        event_loop.run(move |event, _target, control_flow| match event {
            Event::NewEvents(_) => {
                let now = Instant::now();
                imgui.io_mut().update_delta_time(now - last_frame);
                last_frame = now;
            }
            Event::MainEventsCleared => {
                let gl_window = display.gl_window();
                platform
                    .prepare_frame(imgui.io_mut(), &gl_window.window())
                    .expect("Failed to prepare frame");
                gl_window.window().request_redraw();
            }
            Event::RedrawRequested(_) => {
                let mut ui = imgui.frame();
                let gl_window = display.gl_window();
                let size = gl_window.window().inner_size();

                let mut run = true;
                run_ui(&mut run, &mut ui, size.width, size.height);
                if !run {
                    *control_flow = ControlFlow::Exit;
                }

                let mut target = display.draw();
                target.clear_color_srgb(1., 1., 1., 1.);
                platform.prepare_render(&ui, gl_window.window());
                let draw_data = ui.render();
                renderer
                    .render(&mut target, draw_data)
                    .expect("Rendering failed");
                target.finish().expect("Failed to swap buffers");
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            event => {
                let gl_window = display.gl_window();
                platform.handle_event(imgui.io_mut(), gl_window.window(), &event);
            }
        })
    }
}
