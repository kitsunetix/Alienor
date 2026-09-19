use serde_json::Value;

pub(crate) enum CommandName {
    LoadUrl,
    Play,
    Pause,
    Seek,
    SetOffset,
    GetOffset,
    SetLoop,
    GetLoop,
    Unknown(String),
}

impl CommandName {
    pub(crate) fn from_json(value: &Value) -> Option<Self> {
        let command = value.get("command")?.as_str()?;

        Some(match command {
            "loadURL" => Self::LoadUrl,
            "play" => Self::Play,
            "pause" => Self::Pause,
            "seek" => Self::Seek,
            "setOffset" => Self::SetOffset,
            "getOffset" => Self::GetOffset,
            "setLoop" => Self::SetLoop,
            "getLoop" => Self::GetLoop,
            other => Self::Unknown(other.to_string()),
        })
    }
}
