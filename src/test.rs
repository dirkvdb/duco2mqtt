
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;

#[derive(Debug, Deserialize, Serialize)]
struct Node {
    Node: u32,
    General: GeneralInfo,
    Ventilation: VentilationInfo,
    Sensor: Option<SensorInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
struct GeneralInfo {
    Type: TypeVal,
    SubType: SubTypeVal,
    NetworkType: NetworkTypeVal,
    Parent: ParentVal,
    Asso: AssoVal,
    Name: NameVal,
    Identify: IdentifyVal,
}

#[derive(Debug, Deserialize, Serialize)]
struct VentilationInfo {
    State: StateVal,
    TimeStateRemain: TimeStateRemainVal,
    TimeStateEnd: TimeStateEndVal,
    Mode: ModeVal,
    FlowLvlTgt: FlowLvlTgtVal,
}

#[derive(Debug, Deserialize, Serialize)]
struct SensorInfo {
    IaqCo2: IaqCo2Val,
}

#[derive(Debug, Deserialize, Serialize)]
struct TypeVal {
    Val: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct SubTypeVal {
    Val: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct NetworkTypeVal {
    Val: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct ParentVal {
    Val: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct AssoVal {
    Val: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct NameVal {
    Val: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct IdentifyVal {
    Val: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct StateVal {
    Val: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct TimeStateRemainVal {
    Val: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct TimeStateEndVal {
    Val: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct ModeVal {
    Val: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct FlowLvlTgtVal {
    Val: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct IaqCo2Val {
    Val: u32,
}

fn main() {
    // Read the JSON file
    let mut file = File::open("/Users/dirk/Projects/duco2mqtt/test/data/info_nodes.json").unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();

    // Deserialize the JSON into a Vec<Node>
    let nodes: Vec<Node> = serde_json::from_str(&contents).unwrap();

    // Print the deserialized nodes
    for node in nodes {
        println!("{:?}", node);
    }
}
