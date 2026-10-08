use midir::{MidiInputConnection, MidiOutputConnection};
use serde::{Deserialize, Serialize};
use servo_base::generic_channel::GenericSender;
use servo_base::id::PipelineId;

pub struct CallbackData {
    pub port_id: String,
    pub sender: GenericSender<MIDIMsg>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MidiPort {
    Input(MidiPortInfo),
    Output(MidiPortInfo),
}

impl MidiPort {
    pub fn is_input(&self) -> bool {
        match self {
            MidiPort::Input(_) => true,
            MidiPort::Output(_) => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MidiPortInfo {
    pub id: String,
    pub name: String,
}

pub enum MidiConnection {
    Input(Option<MidiInputConnection<CallbackData>>),
    Output(Option<MidiOutputConnection>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MIDIMsg {
    Data(PipelineId, String, Vec<u8>),
    ConnectionEvent,
    PortOpened(PipelineId, String),
    PortClosed(PipelineId, String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MIDIRequest {
    GetPorts(GenericSender<Vec<MidiPort>>),
    OpenPort(PipelineId, String),
    ClosePort(PipelineId, String),
    SendData(String, Vec<u8>),
}
