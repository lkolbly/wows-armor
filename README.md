World of Warships Artillery Calculator
======================================

Getting Started
---------------

If you have Rust installed, you can simply run:
```
$ cargo build --release
```
to build an executable. You can then run the executable found in `target/release/wows_armor` to run the calculations (note: see below about running with "info").

The first time you run the executable, it will download all of the metadata, cache HTTP requests in the `cache/` directory, and then cache all of the ship metadata in the `ships.dat` file. If you edit any of the ship data structs in the source code you will need to delete and recreate the `ships.dat` file.

Debugging
---------
For debugging, you can change the logging level using:
```
$ RUST_LOG=wows_armor=trace cargo run
```
Valid levels include `trace`, `debug`, `info`, `warn`, and `error`. The default is `warn`.

Note that when running out of the box, all of the calculated results are computed at the `info` level, so you should run using:
```
$ RUST_LOG=wows_armor=info ./target/release/wows_armor
```
to see the full output.
