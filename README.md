# Revy

[![Latest version](https://img.shields.io/crates/v/revy.svg)](https://crates.io/crates/revy)
[![Documentation](https://docs.rs/revy/badge.svg)](https://docs.rs/revy)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/rerun-io/revy/blob/master/LICENSE-MIT)
[![Apache](https://img.shields.io/badge/license-Apache-blue.svg)](https://github.com/rerun-io/revy/blob/master/LICENSE-APACHE)

Revy is a proof-of-concept time-travel debugger for the [Bevy](https://github.com/bevyengine/bevy) game engine, built using [Rerun](https://github.com/rerun-io/rerun).

The general idea is that one would use Revy to investigate gameplay/physics/general-behavior-ish kinds of bugs.  
Revy is _not_ a graphics debugger: for that you'd use e.g. [RenderDoc](https://github.com/baldurk/renderdoc).  
It is _not_ a performance profiler either: for that, Bevy integrates well with e.g. [Tracy](https://github.com/wolfpld/tracy).

Revy works by snapshotting diffs of the Bevy database every frame that are then logged into the Rerun database.  
This allows you to inspect and visualize the state of the engine at any point in time, either in real-time or after the fact.  
These recordings can then be shared to be replayed or e.g. attached to bug reports.

https://github.com/rerun-io/revy/assets/2910679/cd096cbe-5e68-4acf-8010-e6c32c5568dc

## Examples 

TODO: fix links & screenshots.

|[Breakout](https://github.com/bevyengine/bevy/blob/v0.15.0/examples/games/breakout.rs)|[3D shapes](https://github.com/bevyengine/bevy/blob/v0.15.0/examples/3d/3d_shapes.rs)|[Alien Cake Addict](https://github.com/bevyengine/bevy/blob/v0.15.0/examples/games/alien_cake_addict.rs)|
| :----------------------------------------------------------------: | :-------------------------------------------------------------: | :--------------------------------------------------------------: |
| [*Live demo*](https://app.rerun.io/version/0.14.1/index.html?url=https://storage.googleapis.com/rerun-example-datasets/revy/breakout_014_001.rrd) | [*Live demo*](https://app.rerun.io/version/0.14.1/index.html?url=https://storage.googleapis.com/rerun-example-datasets/revy/3d_shapes_014_001.rrd)| [*Live demo*](https://app.rerun.io/version/0.14.1/index.html?url=https://storage.googleapis.com/rerun-example-datasets/revy/alien_014_001.rrd) |
| <picture> <img src="https://static.rerun.io/revy_breakout_title/a853af41115505212296813a0bef2373b105757b/full.png" alt=""> <source media="(max-width: 480px)" srcset="https://static.rerun.io/revy_breakout_title/a853af41115505212296813a0bef2373b105757b/480w.png"> <source media="(max-width: 768px)" srcset="https://static.rerun.io/revy_breakout_title/a853af41115505212296813a0bef2373b105757b/768w.png"> <source media="(max-width: 1024px)" srcset="https://static.rerun.io/revy_breakout_title/a853af41115505212296813a0bef2373b105757b/1024w.png"> <source media="(max-width: 1200px)" srcset="https://static.rerun.io/revy_breakout_title/a853af41115505212296813a0bef2373b105757b/1200w.png"> </picture> | <picture> <img src="https://static.rerun.io/revy_3dshapes_title/964446d03f1792b60e394e8c495e6fe16273939a/full.png" alt=""> <source media="(max-width: 480px)" srcset="https://static.rerun.io/revy_3dshapes_title/964446d03f1792b60e394e8c495e6fe16273939a/480w.png"> <source media="(max-width: 768px)" srcset="https://static.rerun.io/revy_3dshapes_title/964446d03f1792b60e394e8c495e6fe16273939a/768w.png"> <source media="(max-width: 1024px)" srcset="https://static.rerun.io/revy_3dshapes_title/964446d03f1792b60e394e8c495e6fe16273939a/1024w.png"> <source media="(max-width: 1200px)" srcset="https://static.rerun.io/revy_3dshapes_title/964446d03f1792b60e394e8c495e6fe16273939a/1200w.png"> </picture>  | <picture> <img src="https://static.rerun.io/revy_alien_title/3e4ba4f3cfb728942ecb38ba3e613f3498dda3e2/full.png" alt=""> <source media="(max-width: 480px)" srcset="https://static.rerun.io/revy_alien_title/3e4ba4f3cfb728942ecb38ba3e613f3498dda3e2/480w.png"> <source media="(max-width: 768px)" srcset="https://static.rerun.io/revy_alien_title/3e4ba4f3cfb728942ecb38ba3e613f3498dda3e2/768w.png"> <source media="(max-width: 1024px)" srcset="https://static.rerun.io/revy_alien_title/3e4ba4f3cfb728942ecb38ba3e613f3498dda3e2/1024w.png"> <source media="(max-width: 1200px)" srcset="https://static.rerun.io/revy_alien_title/3e4ba4f3cfb728942ecb38ba3e613f3498dda3e2/1200w.png"> </picture> |


---

:warning: _This is not an official Rerun project. This is a side-experiment meant to explore the possibilities that a tool like Rerun could open when it comes to gamedev. It is not a full-fledged, properly maintained thing -- nor does it aim to be. It's also probably buggy and slow in many ways, and it certainly is full of code abominations :upside_down_face:._ 

## Usage

1. [Install the Rerun Viewer](https://www.rerun.io/docs/getting-started/installing-viewer) (`0.17`).

2. Add `revy` to your dependencies:
    ```toml
    revy = "0.20"  # always matches the rerun version
    ```

3. Initialize the `rerun` plugin:
    ```rust
    .add_plugins({
        let rec = revy::RecordingStreamBuilder::new("<your_app_name>").spawn().unwrap();
        revy::RerunPlugin { rec }
    })
    ```
    This will start a Rerun Viewer in the background and stream the recording data to it.  
    Check out the [`RecordingStreamBuilder`](https://docs.rs/rerun/latest/rerun/struct.RecordingStreamBuilder.html) docs for other options (saving to file, connecting to a remote viewer, etc).

## Examples

This repository comes with a number of pre-injected Bevy examples:

```shell
cargo run --example breakout
cargo run --example alien_cake_addict
```


## Custom loggers

Revy will record every components of every single entity (), either using one of the builtin [dedicated loggers](./src/default_loggers.rs), or using the generic reflection-based logger.

You can also register your own custom loggers by inserting a `RerunComponentLoggers` resource:
```rust
.insert_resource(revy::RerunComponentLoggers::new([
    (
        "bevy_render::view::visibility::ViewVisibility".into(),
        Some(revy::RerunLogger::new(
            |_world, _all_entities, entity, _component| {
                let suffix = None;

                use revy::external::rerun;
                let data = entity
                    .get::<ViewVisibility>()
                    .map(|vviz| {
                        revy::Aliased::<rerun::components::Text>::new(
                            "ViewVisibility",
                            rerun::components::Text(
                                if vviz.get() { ":)))" } else { ":'(" }.into(),
                            ),
                        )
                    })
                    .map(|data| Box::new(data) as _);

                (suffix, data)
            },
        )),
    ),
]))
```

## Compatibility

| Bevy                                                             | Revy                                                          | Rerun                                                          |
| ---------------------------------------------------------------- | ------------------------------------------------------------- | -------------------------------------------------------------- |
| [0.13](https://github.com/bevyengine/bevy/releases/tag/v0.13.0)  | [0.14](https://github.com/rerun-io/revy/releases/tag/0.14.0)  | [0.14](https://github.com/rerun-io/rerun/releases/tag/0.14.0)  |
| [0.13](https://github.com/bevyengine/bevy/releases/tag/v0.13.0)  | [0.15](https://github.com/rerun-io/revy/releases/tag/0.15.0)  | [0.15](https://github.com/rerun-io/rerun/releases/tag/0.15.0)  |
| [0.13](https://github.com/bevyengine/bevy/releases/tag/v0.13.0)  | [0.16](https://github.com/rerun-io/revy/releases/tag/0.16.0)  | [0.16](https://github.com/rerun-io/rerun/releases/tag/0.16.0)  |
| [0.14](https://github.com/bevyengine/bevy/releases/tag/v0.14.0)  | [0.17](https://github.com/rerun-io/revy/releases/tag/0.17.0)  | [0.17](https://github.com/rerun-io/rerun/releases/tag/0.17.0)  |
| [0.15](https://github.com/bevyengine/bevy/releases/tag/v0.15.0)  | [0.20](https://github.com/rerun-io/revy/releases/tag/0.20.0)  | [0.20](https://github.com/rerun-io/rerun/releases/tag/0.20.0)  |
