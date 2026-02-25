# clap-host

[![CI](https://github.com/PoHsuanLai/clap-host/actions/workflows/ci.yml/badge.svg)](https://github.com/PoHsuanLai/clap-host/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/clap-host.svg)](https://crates.io/crates/clap-host)
[![docs.rs](https://docs.rs/clap-host/badge.svg)](https://docs.rs/clap-host)
[![License](https://img.shields.io/crates/l/clap-host.svg)](LICENSE)

A safe Rust library for hosting [CLAP](https://cleveraudio.org/) audio plugins.

Handles plugin loading, audio processing, MIDI, parameters, transport, state, GUI, and 30+ CLAP extensions -- all behind a safe, ergonomic API.

## Quick Start

```rust
use clap_host::{ClapInstance, MidiEvent, ProcessContext, TransportInfo};

// Load a plugin
let mut plugin = ClapInstance::load("/path/to/plugin.clap", 44100.0, 512)?;
println!("Loaded: {}", plugin.info()); // "Diva v1.4 by u-he"

// Process audio
let transport = TransportInfo::new().with_tempo(128.0).with_playing(true);
let output = plugin.process(&mut buffer, &ProcessContext {
    midi: &[MidiEvent::note_on(0, 0, 60, 100)],
    transport: Some(&transport),
    ..Default::default()
})?;
```

## Features

- **Cross-platform** -- macOS, Linux, Windows
- **f32 and f64** audio processing
- **MIDI** -- note on/off, CC, pitch bend, program change, poly pressure, sysex
- **Parameters** -- enumerate, get/set, automation with sample-accurate timing
- **Note expression** -- MPE-style per-note volume, pan, tuning, vibrato, brightness
- **Transport** -- tempo, time signature, play/record state, loop points, bar position
- **State** -- save/load plugin state with optional context (preset, project, duplicate)
- **GUI** -- open/close plugin editor windows
- **30+ extensions** -- audio ports, note ports, ambisonic, surround, voice info, undo, triggers, tuning, remote controls, context menus, and more

## Usage

### Parameters

```rust
// Query
let params = plugin.get_all_parameters();
for p in &params {
    println!("{}: {} [{}, {}]", p.id, p.name, p.min_value, p.max_value);
}

// Set (chainable)
plugin
    .set_parameter(0, 0.75)
    .set_parameter(1, 0.5);
```

### Transport

```rust
let transport = TransportInfo::new()
    .with_tempo(140.0)
    .with_playing(true)
    .with_time_signature(3, 4)
    .with_position(8.0, 3.43)
    .with_loop(true, 4.0, 16.0);
```

### Note Expression

```rust
use clap_host::{NoteExpressionValue, NoteExpressionType};

let expr = NoteExpressionValue::new(NoteExpressionType::Tuning, /*note_id*/ 0, 0.5)
    .at(128)        // sample offset
    .on_channel(0);
```

### State

```rust
// Save
let state = plugin.save_state()?;

// Load
plugin.load_state(&state)?;

// With context
use clap_host::StateContext;
let preset = plugin.save_state_with_context(StateContext::ForPreset)?;
```

### Event Lists

```rust
use clap_host::InputEventList;

let mut events = InputEventList::new();
events
    .add_midi_events(&midi)
    .add_param_changes(&param_changes)
    .add_note_expressions(&expressions)
    .sort_by_time();
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| [clap-sys](https://crates.io/crates/clap-sys) | CLAP FFI bindings |
| [libloading](https://crates.io/crates/libloading) | Dynamic library loading |
| [bitflags](https://crates.io/crates/bitflags) | Flag types (ParameterFlags, AudioPortFlags, etc.) |
| [smallvec](https://crates.io/crates/smallvec) | Stack-allocated parameter queues |
| [thiserror](https://crates.io/crates/thiserror) | Error types |

## License

MIT OR Apache-2.0
