#!/usr/bin/env bash


cd "$(dirname "$0")" || exit

RUSTFLAGS="-C target-cpu=native" cargo bench

./summarise_bench.sh > bench_optimisations.txt