use std::collections::BTreeMap;
use std::env::current_exe;
use std::fs::File;
use std::io::{self, BufRead};
use std::process::exit;

use regex::Regex;
use swayipc::{Connection, Event, EventType, WindowChange};

fn main() {
    // Get the path to the icons data file
    let icons_path = current_exe()
        .expect("[Workspace renamer] Failed to get executable path")
        .parent()
        .unwrap()
        .join("sway_icons.txt");
    let file = File::open(icons_path).expect("[Workspace renamer] Failed to open icons file");
    let icons: BTreeMap<String, String> = io::BufReader::new(file)
        .lines()
        .filter_map(|line| {
            let line = line.expect("[Workspace renamer] Failed to read line from icons file");
            if let Some((name, icon)) = line.split_once(' ') {
                Some((name.to_string(), icon.to_string()))
            } else {
                println!("[Workspace renamer] Malformed line in icons file: {}", line);
                None
            }
        })
        .collect();

    // Establish connection to Sway IPC
    let mut connection = Connection::new()
        .expect("[Workspace renamer] Failed to establish a connection to Sway IPC");

    // Second connection to listen for events
    let subscribe_connection = Connection::new()
        .expect("[Workspace renamer] Failed to establish a connection to Sway IPC");

    // Subscribe to window events
    let event_stream = subscribe_connection
        .subscribe([EventType::Window])
        .expect("[Workspace renamer] Failed to subscribe to events");

    let window_regex = Regex::new(r"[a-zA-Z0-9://_\-\.][a-zA-Z0-9://_\-\.]+").expect("Bad regex");

    // Main loop - process events
    for event in event_stream {
        match event {
            Ok(Event::Window(window_event)) => {
                match window_event.change {
                    // Match events that could require workspace renaming
                    WindowChange::New | WindowChange::Close | WindowChange::Move => {
                        // Create a command to rename each workspace
                        let commands: Vec<String> = connection
                            .get_workspaces()
                            .expect("[Workspace renamer] Failed to get workspaces")
                            .into_iter()
                            .map(|workspace| {
                                let number = workspace.num;
                                let representation = workspace
                                    .representation
                                    .expect("[Workspace renamer] Workspace has no representation");
                                let window_names: Vec<&str> = window_regex
                                    .find_iter(&representation)
                                    .map(|m| {
                                        let m = m.as_str();
                                        // Map to an icon, if one is available
                                        icons.get(m).map(|icon| icon.as_str()).unwrap_or(m)
                                    })
                                    .collect();
                                if window_names.is_empty() {
                                    format!("rename workspace number {} to {}", number, number)
                                } else {
                                    format!(
                                        "rename workspace number {} to '{} {}'",
                                        number,
                                        number,
                                        window_names.join(" ")
                                    )
                                }
                            })
                            .collect();

                        // Execute the commands
                        for command in commands {
                            connection
                                .run_command(command)
                                .expect("[Workspace renamer] Failed to execute command");
                        }
                    }
                    // Ignore other events
                    _ => {}
                }
            }
            Err(_) => {
                // An error here indicates that sway has exited, so we should temrinate
                exit(0);
            }
            Ok(_) => {
                panic!(
                    "[Workspace renamer] ERROR: Received an event we did not subscribe to: {:?}",
                    event
                );
            }
        }
    }
}
