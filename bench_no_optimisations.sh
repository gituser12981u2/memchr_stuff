#!/usr/bin/env bash


cd "$(dirname "$0")" || exit
cargo bench

./summarise_bench.sh > bench_no_optimisations.txt