use super::error::Error;
use super::state::MpvPlayer;

impl MpvPlayer {
    pub fn set_property(&self, name: &str, value: &str) -> Result<(), Error> {
        let handle = self.handle.lock()?;

        if name == "pause" {
            let bool_value = match value {
                "yes" | "true" => true,
                "no" | "false" => false,
                _ => {
                    return Err(Error::PropertyError(
                        format!("Invalid pause value: {}", value),
                        -1,
                    ))
                }
            };

            return handle.set_property(name, bool_value).map_err(|e| {
                Error::PropertyError(
                    format!("Failed to set property {} to {}: {}", name, value, e),
                    -1,
                )
            });
        }

        handle.set_property(name, value).map_err(|e| {
            Error::PropertyError(
                format!("Failed to set property {} to {}: {}", name, value, e),
                -1,
            )
        })
    }

    pub fn command(&self, cmd: &str, args: &[&str]) -> Result<(), Error> {
        let handle = self.handle.lock()?;

        if cmd == "set" && args.len() >= 2 && args[0] == "pause" {
            let pause_value = match args[1] {
                "yes" | "true" => true,
                "no" | "false" => false,
                _ => {
                    return Err(Error::CommandError(
                        format!("Invalid pause value: {}", args[1]),
                        -1,
                    ))
                }
            };

            return handle.set_property("pause", pause_value).map_err(|e| {
                Error::CommandError(format!("Failed to set pause to {}: {}", args[1], e), -1)
            });
        }

        handle.command(cmd, args).map_err(|e| {
            Error::CommandError(format!("Failed to execute command {}: {}", cmd, e), -1)
        })
    }

    pub fn load_file(&self, file_path: &str) -> Result<(), Error> {
        println!("Loading file: {}", file_path);
        self.command("loadfile", &[file_path])?;

        let offset = self.get_offset_seconds();
        if offset.abs() > 0.05 {
            std::thread::sleep(std::time::Duration::from_millis(200));
            println!(
                "Applying significant offset after load: seeking to {} seconds",
                offset
            );
            self.command("seek", &[&offset.to_string(), "absolute", "exact"])?;
        } else {
            println!(
                "Offset near zero ({:.3}s), letting MPV start normally.",
                offset
            );
        }

        Ok(())
    }

    pub fn set_loop(&self, enabled: bool) -> Result<(), Error> {
        let value = if enabled { "inf" } else { "no" };
        println!("Setting loop to: {}", value);

        {
            let handle = self.handle.lock()?;
            handle.set_property("loop-file", value)?;
        }
        {
            let handle = self.handle.lock()?;
            handle.set_property("loop-playlist", value)?;
        }

        Ok(())
    }

    pub fn get_loop(&self) -> Result<bool, Error> {
        let handle = self.handle.lock()?;

        let loop_state = handle
            .get_property::<String>("loop-file")
            .map_err(|e| Error::PropertyError(format!("Failed to get loop state: {}", e), -1))?;

        Ok(loop_state != "no")
    }
}
