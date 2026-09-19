use serde_json::Value;

pub(crate) enum CommandRequest {
    LoadUrl(String),
    Play,
    Pause,
    Seek(f64),
    SetOffset {
        seconds: Option<f64>,
        frames: Option<i64>,
        fps: f64,
    },
    GetOffset,
    SetLoop(bool),
    GetLoop,
    Invalid {
        command: String,
        message: &'static str,
    },
    Unknown(String),
}

impl CommandRequest {
    pub(crate) fn from_json(value: &Value) -> Option<Self> {
        let command = value.get("command")?.as_str()?;

        Some(match command {
            "loadURL" => match value.get("url").and_then(Value::as_str) {
                Some(url) => Self::LoadUrl(url.to_string()),
                None => Self::Invalid {
                    command: command.to_string(),
                    message: "Missing or invalid 'url' field",
                },
            },
            "play" => Self::Play,
            "pause" => Self::Pause,
            "seek" => match value.get("position").and_then(Value::as_f64) {
                Some(position) => Self::Seek(position),
                None => Self::Invalid {
                    command: command.to_string(),
                    message: "Missing or invalid 'position' field",
                },
            },
            "setOffset" => {
                let seconds = value.get("seconds").and_then(Value::as_f64);
                let frames = value.get("frames").and_then(Value::as_i64);
                let fps = value.get("fps").and_then(Value::as_f64).unwrap_or(30.0);

                if seconds.is_some() || frames.is_some() {
                    Self::SetOffset {
                        seconds,
                        frames,
                        fps,
                    }
                } else {
                    Self::Invalid {
                        command: command.to_string(),
                        message: "Command error (-1): Invalid offset parameters",
                    }
                }
            }
            "getOffset" => Self::GetOffset,
            "setLoop" => match value.get("enabled").and_then(Value::as_bool) {
                Some(enabled) => Self::SetLoop(enabled),
                None => Self::Invalid {
                    command: command.to_string(),
                    message: "Missing or invalid 'enabled' field",
                },
            },
            "getLoop" => Self::GetLoop,
            other => Self::Unknown(other.to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::CommandRequest;
    use serde_json::json;

    #[test]
    fn parses_seek_request() {
        match CommandRequest::from_json(&json!({
            "command": "seek",
            "position": 12.5
        })) {
            Some(CommandRequest::Seek(position)) => assert_eq!(position, 12.5),
            _ => panic!("expected seek request"),
        }
    }

    #[test]
    fn rejects_seek_without_position() {
        match CommandRequest::from_json(&json!({ "command": "seek" })) {
            Some(CommandRequest::Invalid { command, message }) => {
                assert_eq!(command, "seek");
                assert_eq!(message, "Missing or invalid 'position' field");
            }
            _ => panic!("expected invalid seek request"),
        }
    }

    #[test]
    fn parses_frame_offset_with_default_fps() {
        match CommandRequest::from_json(&json!({
            "command": "setOffset",
            "frames": 30
        })) {
            Some(CommandRequest::SetOffset {
                seconds: None,
                frames: Some(30),
                fps,
            }) => assert_eq!(fps, 30.0),
            _ => panic!("expected frame offset request"),
        }
    }

    #[test]
    fn preserves_unknown_command_name() {
        match CommandRequest::from_json(&json!({ "command": "futureCommand" })) {
            Some(CommandRequest::Unknown(command)) => assert_eq!(command, "futureCommand"),
            _ => panic!("expected unknown command"),
        }
    }
}
