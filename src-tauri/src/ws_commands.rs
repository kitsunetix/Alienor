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
