use portpicker::pick_unused_port;

pub(crate) fn select_port(configured_port: Option<u16>) -> u16 {
    match configured_port {
        Some(port) if (1025..=65535).contains(&port) => {
            match std::net::TcpListener::bind(format!("0.0.0.0:{}", port)) {
                Ok(listener) => {
                    drop(listener);
                    port
                }
                Err(error) => {
                    eprintln!(
                        "Configured port {} is unavailable: {}. Falling back.",
                        port, error
                    );
                    pick_unused_port().expect("No ports available")
                }
            }
        }
        Some(port) => {
            eprintln!("Configured port {} is invalid. Falling back.", port);
            pick_unused_port().expect("No ports available")
        }
        None => match std::net::TcpListener::bind(("0.0.0.0", 3000)) {
            Ok(listener) => {
                drop(listener);
                3000
            }
            Err(_) => pick_unused_port().expect("No ports available"),
        },
    }
}
