#!/usr/bin/env bash
set -euo pipefail

arguments=("$@")
runtime_path=""
for index in "${!arguments[@]}"; do
    case "${arguments[index]}" in
        -L)
            library_dir=${arguments[index + 1]}
            ;;
        -L*)
            library_dir=${arguments[index]#-L}
            ;;
        *)
            continue
            ;;
    esac
    if [[ -f "$library_dir/libsima_runtime.so" ]]; then
        runtime_path="$library_dir/libsima_runtime.so"
        break
    fi
done

if [[ -n "$runtime_path" ]]; then
    for index in "${!arguments[@]}"; do
        case "${arguments[index]}" in
            */libstd-*.so)
                arguments[index]="$runtime_path"
                ;;
        esac
    done
fi

exec "${SIMA_LINKER:-cc}" "${arguments[@]}"
