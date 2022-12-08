#!/bin/sh

cargo b --release && sudo RUST_BACKTRACE=1 ./target/release/yclass
