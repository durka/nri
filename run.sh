#!/bin/bash

export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:./crates/drivers/optoforce
export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:./crates/drivers/biotac/src/wrapper
export RUST_BACKTRACE=1

if [ "$#" -eq 0 ]; then 
    rlwrap cargo run --release
    #target/release/nri
elif [ "$1" == "all" ]; then
    DIR="$2"
    if [ -e "$DIR/teensy.dat" ]; then
        echo -e "\nProcessing all data files in $DIR\n"
        if [ ! -e "$DIR/teensy.ft.csv" ]; then
            $0 teensy "$DIR/teensy.dat"
        fi
        if [ -e "$DIR/optoforce.dat" ] && [ ! -e "$DIR/optoforce.csv" ]; then
            $0 optoforce "$DIR/optoforce.dat"
        fi
        if [ -e "$DIR/biotac.dat" ] && [ ! -e "$DIR/biotac.csv" ]; then
            $0 biotac "$DIR/biotac.dat"
        fi
        if [ -e "$DIR/structure_times.csv" ] && [ ! -e "$DIR/structure" ]; then
            $0 structure "$DIR/structure_times.csv"
        fi
        if [ -e "$DIR/bluefox_times.csv" ] && [ ! -e "$DIR/bluefox" ]; then
            $0 bluefox "$DIR/bluefox_times.csv"
        fi
    else
        DIRS=$(find "$DIR" -mindepth 1 -maxdepth 1 -type d)
        if [ ! -z "$DIRS" ]; then
            echo -e "\nProcessing all data directories in $DIR\n"
            find "$DIR" -mindepth 1 -maxdepth 1 -type d -exec "$0" all {} \;
        fi
    fi
else
    DEV=$1
    shift
    rlwrap cargo run --release --example read$DEV -- "$@"
    #target/release/examples/read$DEV "$@"
fi

