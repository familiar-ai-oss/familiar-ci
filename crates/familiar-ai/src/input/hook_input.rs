//! Parse the hook's input, one JSON object on stdin with the common fields and
//! the event's own. Unknown fields are ignored, never refused, and `tool_input`
//! and the tool's result stay raw JSON.

use serde::Deserialize;
use serde_json::Value;

/// Claude Code's event names, exactly.
pub const EVENT_NAMES: [&str; 10] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Stop",
    "SubagentStop",
    "SubagentStart",
    "PreModelSwitch",
    "PostModelSwitch",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The events Claude Code calls the hook for.
pub enum HookEvent {
    SessionStart,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    /// A tool call that did not return normally (an error, Esc); hooked for the spawn only [draft].
    PostToolUseFailure,
    Stop,
    SubagentStop,
    SubagentStart,
    PreModelSwitch,
    PostModelSwitch,
}

impl HookEvent {
    pub fn name(self) -> &'static str {
        match self {
            HookEvent::SessionStart => "SessionStart",
            HookEvent::UserPromptSubmit => "UserPromptSubmit",
            HookEvent::PreToolUse => "PreToolUse",
            HookEvent::PostToolUse => "PostToolUse",
            HookEvent::PostToolUseFailure => "PostToolUseFailure",
            HookEvent::Stop => "Stop",
            HookEvent::SubagentStop => "SubagentStop",
            HookEvent::SubagentStart => "SubagentStart",
            HookEvent::PreModelSwitch => "PreModelSwitch",
            HookEvent::PostModelSwitch => "PostModelSwitch",
        }
    }

    pub fn from_name(name: &str) -> Option<HookEvent> {
        Some(match name {
            "SessionStart" => HookEvent::SessionStart,
            "UserPromptSubmit" => HookEvent::UserPromptSubmit,
            "PreToolUse" => HookEvent::PreToolUse,
            "PostToolUse" => HookEvent::PostToolUse,
            "PostToolUseFailure" => HookEvent::PostToolUseFailure,
            "Stop" => HookEvent::Stop,
            "SubagentStop" => HookEvent::SubagentStop,
            "SubagentStart" => HookEvent::SubagentStart,
            "PreModelSwitch" => HookEvent::PreModelSwitch,
            "PostModelSwitch" => HookEvent::PostModelSwitch,
            _ => return None,
        })
    }

    /// The clap value parser; any name not in `EVENT_NAMES` is a usage error.
    pub fn parse_cli(name: &str) -> Result<HookEvent, String> {
        HookEvent::from_name(name).ok_or_else(|| {
            format!(
                "unknown event {name:?}; expected one of: {}",
                EVENT_NAMES.join(", ")
            )
        })
    }
}

/// Why stdin was refused; every variant fails closed with exit 2.
#[derive(Debug, PartialEq, Eq)]
pub enum InputError {
    NotJson(String),
    NotAnObject,
    /// `hook_event_name` was missing or not one of `EVENT_NAMES`.
    MissingEvent,
    /// The event named on the command line and `hook_event_name` disagree.
    EventMismatch { cli: String, stdin: String },
}

impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputError::NotJson(e) => write!(f, "stdin is not valid JSON: {e}"),
            InputError::NotAnObject => write!(f, "stdin is not one JSON object"),
            InputError::MissingEvent => {
                write!(f, "stdin has no usable hook_event_name")
            }
            InputError::EventMismatch { cli, stdin } => write!(
                f,
                "event mismatch: command line says {cli}, stdin says {stdin}"
            ),
        }
    }
}

/// One hook call's input: the common fields plus every event's own fields.
/// `event` is set by `HookInput::parse` once the command line and
/// `hook_event_name` agree.
#[derive(Debug, Clone)]
pub struct HookInput {
    /// The event, agreed by the command line and `hook_event_name`.
    pub event: HookEvent,

    // Common fields.
    pub hook_event_name: String,
    pub session_id: Option<String>,
    pub transcript_path: Option<String>,
    pub cwd: Option<String>,
    pub permission_mode: Option<String>,
    pub prompt_id: Option<String>,

    // SessionStart.
    pub source: Option<String>,
    pub model: Option<String>,

    // UserPromptSubmit, which also delivers a finished background task as a prompt.
    pub prompt: Option<String>,

    // PreToolUse and PostToolUse.
    pub tool_name: Option<String>,
    pub tool_use_id: Option<String>,
    /// Raw JSON; the shape depends on the tool.
    pub tool_input: Option<Value>,
    /// The tool's result on PostToolUse, raw JSON shaped by the tool.
    pub tool_response: Option<Value>,

    // PostToolUseFailure, its shape assumed from the Agent SDK's PostToolUseFailureHookInput
    // (not captured): the error as text (an error that is not text kept as its JSON), and
    // whether the person interrupted the call.
    pub error: Option<String>,
    pub is_interrupt: Option<bool>,

    // Stop and SubagentStop.
    pub last_assistant_message: Option<String>,
    pub stop_hook_active: Option<bool>,

    // Present on every event fired from inside a subagent. A subagent shares
    // the session's `session_id`, so `agent_id` is what tells it apart.
    pub agent_id: Option<String>,
    pub agent_type: Option<String>,

    // SubagentStop.
    pub agent_transcript_path: Option<String>,

    // PreModelSwitch and PostModelSwitch; headless hook input names the model only here.
    pub from_model: Option<String>,
    pub to_model: Option<String>,

    /// The conversation this session is when the binary launched it as an
    /// investigator; set only by `HookInput::launched`, never by the input.
    pub conversation: Option<String>,
    /// The binary launched this session (an investigator or a lane), so no
    /// prompt in it is the person's; set only by `HookInput::launched`.
    pub by_binary: bool,
}

/// The serde mirror of `HookInput`, without `event`, which has no honest
/// default and is decided by the agreement check.
#[derive(Deserialize)]
struct RawHookInput {
    hook_event_name: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    transcript_path: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    permission_mode: Option<String>,
    #[serde(default)]
    prompt_id: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    tool_name: Option<String>,
    #[serde(default)]
    tool_use_id: Option<String>,
    #[serde(default)]
    tool_input: Option<Value>,
    #[serde(default)]
    tool_response: Option<Value>,
    #[serde(default)]
    error: Option<Value>,
    #[serde(default)]
    is_interrupt: Option<Value>,
    #[serde(default)]
    last_assistant_message: Option<String>,
    #[serde(default)]
    stop_hook_active: Option<bool>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    agent_type: Option<String>,
    #[serde(default)]
    agent_transcript_path: Option<String>,
    #[serde(default)]
    from_model: Option<String>,
    #[serde(default)]
    to_model: Option<String>,
}

impl RawHookInput {
    fn with_event(self, event: HookEvent) -> HookInput {
        HookInput {
            event,
            hook_event_name: self.hook_event_name,
            session_id: self.session_id,
            transcript_path: self.transcript_path,
            cwd: self.cwd,
            permission_mode: self.permission_mode,
            prompt_id: self.prompt_id,
            source: self.source,
            model: self.model,
            prompt: self.prompt,
            tool_name: self.tool_name,
            tool_use_id: self.tool_use_id,
            tool_input: self.tool_input,
            tool_response: self.tool_response,
            error: self.error.filter(|e| !e.is_null()).map(|e| match e {
                Value::String(s) => s,
                other => other.to_string(),
            }),
            is_interrupt: self.is_interrupt.as_ref().and_then(Value::as_bool),
            last_assistant_message: self.last_assistant_message,
            stop_hook_active: self.stop_hook_active,
            agent_id: self.agent_id,
            agent_type: self.agent_type,
            agent_transcript_path: self.agent_transcript_path,
            from_model: self.from_model,
            to_model: self.to_model,
            conversation: None,
            by_binary: false,
        }
    }
}

/// The separator between the session id and the agent id in an agent conversation id.
pub const AGENT_CONVERSATION_SEPARATOR: char = '/';

/// How a prompt begins when Claude Code, not the person, sent it.
pub const TASK_NOTIFICATION_OPENING: &str = "<task-notification>";

fn is_id_part(text: &str) -> bool {
    !text.is_empty() && !text.contains(AGENT_CONVERSATION_SEPARATOR)
}

impl HookInput {
    /// Parse stdin and check it names the same event as the command line.
    pub fn parse(stdin_text: &str, cli_event: HookEvent) -> Result<HookInput, InputError> {
        let value: Value =
            serde_json::from_str(stdin_text).map_err(|e| InputError::NotJson(e.to_string()))?;
        if !value.is_object() {
            return Err(InputError::NotAnObject);
        }
        let stdin_event = value
            .get("hook_event_name")
            .and_then(Value::as_str)
            .and_then(HookEvent::from_name)
            .ok_or(InputError::MissingEvent)?;
        if stdin_event != cli_event {
            return Err(InputError::EventMismatch {
                cli: cli_event.name().to_string(),
                stdin: stdin_event.name().to_string(),
            });
        }
        let raw: RawHookInput =
            serde_json::from_value(value).map_err(|e| InputError::NotJson(e.to_string()))?;
        Ok(raw.with_event(cli_event))
    }

    /// The `file_path` inside `tool_input`, or NotebookEdit's `notebook_path`.
    pub fn file_path(&self) -> Option<&str> {
        let input = self.tool_input.as_ref()?;
        input
            .get("file_path")
            .or_else(|| input.get("notebook_path"))
            .and_then(Value::as_str)
    }

    /// The Bash `command` inside `tool_input`.
    pub fn command(&self) -> Option<&str> {
        self.tool_input
            .as_ref()
            .and_then(|v| v.get("command"))
            .and_then(Value::as_str)
    }

    pub fn prompt_text(&self) -> Option<&str> {
        self.prompt.as_deref()
    }

    pub fn tool_output(&self) -> Option<&Value> {
        self.tool_response.as_ref()
    }

    pub fn in_subagent(&self) -> bool {
        self.agent_id.is_some()
    }

    /// The agent conversation this event belongs to: `session_id` for the main
    /// conversation, `session_id/agent_id` for a subagent. None when either part
    /// holds a `/` or there is no session id.
    pub fn agent_conversation_id(&self) -> Option<String> {
        let session = self.session_conversation_id()?;
        match self.agent_id.as_deref() {
            None => Some(session.to_string()),
            Some(agent) if is_id_part(agent) => {
                Some(format!("{session}{AGENT_CONVERSATION_SEPARATOR}{agent}"))
            }
            Some(_) => None,
        }
    }

    /// True when the tool spawns a subagent, named `Agent` in the hooks and
    /// `Task` in `permission_denials` and older builds.
    pub fn is_spawn(&self) -> bool {
        matches!(self.tool_name.as_deref(), Some("Agent") | Some("Task"))
    }

    pub fn input_text(&self, key: &str) -> Option<&str> {
        self.tool_input.as_ref()?.get(key)?.as_str()
    }

    /// The text of one field of the tool's result on PostToolUse.
    pub fn response_text(&self, key: &str) -> Option<&str> {
        self.tool_response.as_ref()?.get(key)?.as_str()
    }

    /// The spawned subagent's conversation, `session_id/agentId`, from
    /// PostToolUse on the Agent tool; nested subagents stay flat.
    pub fn spawned_conversation_id(&self) -> Option<String> {
        let session = self.session_conversation_id()?;
        let agent = self.response_text("agentId").filter(|a| is_id_part(a))?;
        Some(format!("{session}{AGENT_CONVERSATION_SEPARATOR}{agent}"))
    }

    /// This session's main conversation: its session id, or the id the binary
    /// gave a session it launched.
    pub fn session_conversation_id(&self) -> Option<String> {
        let session = self.session_id.as_deref().filter(|s| is_id_part(s))?;
        Some(self.conversation.clone().unwrap_or_else(|| session.to_string()))
    }

    /// Mark a session the binary launched as an investigator, when
    /// `conversation` ends in this session's id: the rules then read it as a
    /// subagent of the holder's session, and its Stop as the investigator's
    /// SubagentStop. Any other value (a lane's own session id) marks only
    /// that the binary launched it.
    pub fn launched(mut self, conversation: Option<&str>, investigator_type: &str) -> HookInput {
        let Some(conv) = conversation else {
            return self;
        };
        self.by_binary = true;
        let own = conv.rsplit_once(AGENT_CONVERSATION_SEPARATOR).map(|(_, last)| last);
        if own.is_none() || own != self.session_id.as_deref() || conv.split(AGENT_CONVERSATION_SEPARATOR).any(str::is_empty) {
            return self;
        }
        self.conversation = Some(conv.to_string());
        if self.event == HookEvent::Stop && self.agent_id.is_none() {
            self.event = HookEvent::SubagentStop;
            self.agent_type = Some(investigator_type.to_string());
        }
        self
    }

    /// True only for a `UserPromptSubmit` the person typed, not a message
    /// Claude Code delivers through that event such as a task notification.
    pub fn is_the_person_speaking(&self) -> bool {
        self.event == HookEvent::UserPromptSubmit
            && self
                .prompt
                .as_deref()
                .is_some_and(|p| !p.trim_start().starts_with(TASK_NOTIFICATION_OPENING))
    }
}

#[cfg(test)]
#[path = "hook_input_tests.rs"]
mod tests;
