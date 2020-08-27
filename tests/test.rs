extern crate futures_signals_structs_traits;
#[macro_use]
extern crate futures_signals_structs_derive;
extern crate futures_signals;

use futures_signals::signal::Mutable;
use futures_signals_structs_traits::{AsMutableStruct, MutableStruct};

#[derive(AsMutableStruct, Debug, PartialEq, Clone)]
struct PlayerScore {
    points: u32,
    multiplier: f32,
}

#[derive(AsMutableStruct, Debug, PartialEq)]
#[MutableStructName = "MyMutableStruct"]
struct CustomNamedStruct {
    level: u8,
}

#[derive(AsMutableStruct, Debug, PartialEq, Clone)]
struct ComposedStruct {
    score: PlayerScore,
    events: Vec<String>,
}

#[test]
fn gets_as_signal() {
    let raw = PlayerScore {
        points: 40,
        multiplier: 1.5
    };
    let player_signal = raw.as_mutable_struct();
    assert_eq!(player_signal.points.get(), 40);

    player_signal.points.set(25);
    assert_eq!(player_signal.points.get(), 25);
}

#[test]
fn gets_snapshot() {
    let raw = PlayerScore {
        points: 40,
        multiplier: 1.5
    };
    let player_signal = raw.as_mutable_struct();
    player_signal.points.set(25);
    let snapshot = player_signal.snapshot();
    assert_eq!(snapshot, PlayerScore {
        points: 25,
        multiplier: 1.5
    });
}

#[test]
fn updates_from_snapshot() {
    let raw = PlayerScore {
        points: 40,
        multiplier: 1.5
    };
    let player_signal = raw.as_mutable_struct();
    let mut snapshot = player_signal.snapshot();
    snapshot.points = 100;
    snapshot.multiplier = 2.0;
    player_signal.update(snapshot);
    assert_eq!(player_signal.snapshot(), PlayerScore {
        points: 100,
        multiplier: 2.0
    });
}

#[test]
fn is_clonable() {
    let raw = PlayerScore {
        points: 40,
        multiplier: 1.5
    };
    let player_signal_1 = raw.as_mutable_struct();
    let player_signal_2 = player_signal_1.clone();
    assert_eq!(player_signal_1.snapshot(), player_signal_2.snapshot());
}

#[test]
fn allows_custom_names() {
    let _unused = MyMutableStruct {
        level: Mutable::new(1u8)
    };
}

#[test]
fn allows_composed_structs() {
    let composed_struct = ComposedStruct {
        score: PlayerScore {
            points: 40,
            multiplier: 0.4
        },
        events: vec!["First".to_string()],
    };
    let mutable_composed_struct = composed_struct.as_mutable_struct();
    mutable_composed_struct.score.points.set(50);
    mutable_composed_struct.events.lock_mut().push_cloned("Second".to_string());
    assert_eq!(mutable_composed_struct.snapshot(), ComposedStruct {
        score: PlayerScore {
            points: 50,
            multiplier: 0.4
        },
        events: vec!["First".to_string(), "Second".to_string()],
    });
}

#[test]
fn updates_composed_structs() {
    let composed_struct = ComposedStruct {
        score: PlayerScore {
            points: 40,
            multiplier: 0.4
        },
        events: vec![],
    };
    let mutable_composed_struct = composed_struct.as_mutable_struct();

    let updated = ComposedStruct {
        score: PlayerScore {
            points: 50,
            multiplier: 0.9
        },
        events: vec!["First".to_string(), "Second".to_string()],
    };
    mutable_composed_struct.update(updated.clone());
    assert_eq!(mutable_composed_struct.snapshot(), updated);
}