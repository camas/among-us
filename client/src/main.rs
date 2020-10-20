use std::{
    thread::JoinHandle,
    time::{Duration, Instant},
};

use client::{Client, ClientSettings, EventHandler, MainServer, ScanSettings};
use common::data::{DisconnectReason, GameListing};

use rand::{prelude::SmallRng, Rng, SeedableRng};

mod gui;

fn main() {
    // Init logging
    flexi_logger::Logger::with_env_or_str("error")
        .start()
        .unwrap();

    // Read command
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        gui();
        return;
    }

    let mode = args.get(1).unwrap();

    match mode.to_ascii_lowercase().as_str() {
        "scan" => scan(),
        "dummy" => dummy(args),
        "wizard" => wizard(args),
        "annoy" => annoy(args),
        other => println!("Unknown command {}", other),
    }
}

fn gui() {
    gui::run();
}

fn scan() {
    let mut total = 0;
    let started = Instant::now();

    let settings = ScanSettings {
        connect_username: "bobby".to_string(),
        ..ScanSettings::default()
    };

    let callback = |listings: Vec<GameListing>| {
        for listing in listings.iter() {
            println!(
                "{:<6} {:<21} {:>2}/{:<2} {:<8} {:<2} {:<6} {}",
                listing.id,
                listing.address,
                listing.player_count,
                listing.max_players,
                format!("{:?}", listing.map_id),
                listing.num_imposters,
                listing.age,
                listing.host_username,
            );
        }
        total += listings.len();
        eprint!("{} games found. {:?} elapsed\r", total, started.elapsed());

        true
    };

    Client::server_scan(settings, callback);
}

fn wizard(args: Vec<String>) {
    if args.len() < 3 {
        println!("Usage: ./client wizard <game_code>");
        return;
    }

    let game_code: String = args.get(2).unwrap().parse().unwrap();
    let handler = WizardHandler {
        last_change: Instant::now(),
        rng: rand::rngs::SmallRng::seed_from_u64(1337),
        has_joined: false,
    };
    let settings = ClientSettings {
        connect_username: "HackerMan".to_string(),
        game_username: "HackerMan".to_string(),
        initial_hat: 12,
        ..ClientSettings::default()
    };
    Client::run_game_code(handler, MainServer::Europe, &game_code, settings);
}

#[derive(Debug)]
struct WizardHandler {
    last_change: Instant,
    has_joined: bool,
    rng: SmallRng,
}

impl WizardHandler {
    fn random_usernames(&mut self, client: &mut Client) {
        let player_ids = client.player_ids.clone();
        for player_id in player_ids {
            let new_name = (0..12)
                .map(|_| if self.rng.gen::<bool>() { '1' } else { '0' })
                .collect::<String>();
            client.set_player_name(player_id, &new_name);
        }
    }

    fn random_colors(&mut self, client: &mut Client) {
        let player_ids = client.player_ids.clone();
        for player_id in player_ids {
            let new_color = self.rng.gen_range(0, 12);
            client.set_player_color(player_id, new_color);
        }
    }
}

impl EventHandler for WizardHandler {
    fn disconnect_reason(&mut self, client: &mut Client, reason: DisconnectReason) {
        println!("Disconnected: {:?}", reason);
        client.disconnect();
    }

    fn joined_game(&mut self, _client: &mut Client) {
        self.has_joined = true;
    }

    fn packet_received(&mut self, client: &mut Client) {
        if !self.has_joined {
            return;
        }

        if self.last_change.elapsed() > Duration::from_millis(200) {
            self.last_change = Instant::now();
            self.random_usernames(client);
            self.random_colors(client);
        }
    }
}

fn annoy(args: Vec<String>) {
    if args.len() < 3 {
        println!("Usage: ./client annoy <game_code>");
        return;
    }

    let game_code: String = args.get(2).unwrap().parse().unwrap();
    let handler = AnnoyHandler {
        has_joined: false,
        last_change: Instant::now(),
    };
    let settings = ClientSettings {
        connect_username: "zero cool".to_string(),
        game_username: "zero cool".to_string(),
        send_initial_info: false,
        // game_scene: "Tutorial".to_string(),
        ..ClientSettings::default()
    };
    Client::run_game_code(handler, MainServer::Europe, &game_code, settings);
}

#[derive(Debug)]
struct AnnoyHandler {
    has_joined: bool,
    last_change: Instant,
}

impl EventHandler for AnnoyHandler {
    fn joined_game(&mut self, client: &mut Client) {
        self.has_joined = true;
        // let ids = client
        //     .net_objects
        //     .player_transforms
        //     .iter()
        //     .map(|obj| obj.net_id())
        //     .collect::<Vec<u32>>();
        // ids.into_iter().for_each(|id| client.delete_net_object(id));
        let game_data = client.net_objects.game_datas.get_mut(0).unwrap();
        //client.delete_net_object(game_data_id);
        game_data.players.values_mut().for_each(|data| {
            data.dirty = true;
            data.name = "katy".to_string();
            data.hat_id = 10;
            data.color = 0;
            data.skin_id = 10;
            data.pet_id = 10;
            //data.is_imposter = true;
        });
        client.update_game_data();
        let host_id = client.host_id.unwrap();
        client.send_chat_player(host_id, "hi every1 im new!!!!!!! *holds up spork* my name is katy but u can call me t3h PeNgU1N oF d00m!!!!!!!! lol…as u can see im very random!!!! thats why i came here, 2 meet random ppl like me ^_^… im 13 years old (im mature 4 my age tho!!) i like 2 watch invader zim w/ my girlfreind (im bi if u dont like it deal w/it) its our favorite tv show!!! bcuz its SOOOO random!!!! shes random 2 of course but i want 2 meet more random ppl =) like they say the more the merrier!!!! lol…neways i hope 2 make alot of freinds here so give me lots of commentses!!!!
DOOOOOMMMM!!!!!!!!!!!!!!!! <--- me bein random again ^_^ hehe…toodles!!!!!

love and waffles,

t3h PeNgU1N oF d00m");
        std::thread::sleep(Duration::from_millis(100));
        client.disconnect();
    }

    fn packet_received(&mut self, _client: &mut Client) {
        if !self.has_joined {
            return;
        }

        if self.last_change.elapsed() > Duration::from_millis(200) {
            self.last_change = Instant::now();
            // let player_ids = client.player_ids.clone();
            // for player_id in player_ids {
            //     client.player_enter_vent(player_id, 0);
            // }
        }
    }
}

fn dummy(args: Vec<String>) {
    if args.len() < 3 {
        println!("Usage: ./client dummy <game_code> [dummy_count]");
        return;
    }

    let game_code: String = args.get(2).unwrap().parse().unwrap();
    let dummy_count = if args.len() < 4 {
        1
    } else {
        args.get(3).unwrap().parse().unwrap()
    };
    let handles: Vec<JoinHandle<()>> = (1..=dummy_count)
        .map(|i| {
            let game_code = game_code.clone();
            std::thread::spawn(move || {
                let handler = DummyHandler {};
                let settings = ClientSettings {
                    game_username: format!("Dummy {}", i),
                    initial_color: 5,
                    initial_hat: 11,
                    ..ClientSettings::default()
                };
                Client::run_game_code(handler, MainServer::Europe, &game_code, settings);
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
}

#[derive(Debug)]
struct DummyHandler {}

impl EventHandler for DummyHandler {
    fn disconnect_reason(&mut self, client: &mut Client, reason: DisconnectReason) {
        println!("Disconnected: {:?}", reason);
        client.disconnect();
    }
}
